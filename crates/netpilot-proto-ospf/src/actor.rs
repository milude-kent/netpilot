use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use std::collections::HashMap;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{Duration, MissedTickBehavior, interval};

use crate::lsdb::Lsdb;
use crate::neighbor::{OspfNeighbor, OspfNeighborState};

#[allow(dead_code)]
pub struct OspfActor {
    name: String,
    table: String,
    router_id: String,
    areas: Vec<String>,
    lsdb: Lsdb,
    neighbors: HashMap<String, OspfNeighbor>,
    state: ProtocolState,
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
}

impl Default for OspfActor {
    fn default() -> Self {
        Self::new()
    }
}

impl OspfActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            table: String::new(),
            router_id: String::new(),
            areas: vec![],
            lsdb: Lsdb::default(),
            neighbors: HashMap::new(),
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            event_tx: None,
        }
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Process a received OSPF Hello packet and update neighbor state.
    #[allow(dead_code)]
    fn handle_hello(&mut self, hello: &netpilot_io::ospf::HelloPacket) {
        let peer_id = netpilot_io::ospf::format_ospf_id(hello.header.router_id);
        let our_id = self.router_id.clone();

        // Find or create the neighbor entry
        let neighbor = self.neighbors.entry(peer_id.clone()).or_insert_with(|| {
            OspfNeighbor::new(&peer_id, "eth0") // interface would come from transport
        });

        // Update neighbor fields from the Hello
        neighbor.priority = hello.router_priority;
        neighbor.dead_timer_secs = hello.dead_interval_secs;

        // Check if our router ID is in the neighbor list → TwoWay
        let our_router_id_u32 = netpilot_io::ospf::parse_ospf_id(&our_id).unwrap_or(0);
        let saw_us = hello.neighbors.contains(&our_router_id_u32);

        let new_state = if saw_us {
            OspfNeighborState::TwoWay
        } else {
            OspfNeighborState::Init
        };

        if neighbor.state != new_state {
            let old = neighbor.state.clone();
            neighbor.state = new_state.clone();
            self.emit(ProtocolEvent::StateChange {
                protocol_name: self.name.clone(),
                new_state: ProtocolState::Up,
                message: format!(
                    "OSPF neighbor {} state {} → {}",
                    peer_id,
                    state_str(&old),
                    state_str(&new_state)
                ),
            });
        }
    }
}

#[allow(dead_code)]
fn state_str(s: &OspfNeighborState) -> &'static str {
    match s {
        OspfNeighborState::Down => "Down",
        OspfNeighborState::Init => "Init",
        OspfNeighborState::TwoWay => "2-Way",
        OspfNeighborState::ExStart => "ExStart",
        OspfNeighborState::Exchange => "Exchange",
        OspfNeighborState::Loading => "Loading",
        OspfNeighborState::Full => "Full",
    }
}

#[async_trait]
impl ProtocolActor for OspfActor {
    fn set_event_tx(&mut self, tx: tokio::sync::broadcast::Sender<ProtocolEvent>) {
        self.event_tx = Some(tx);
    }

    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name;
        self.state = ProtocolState::Start;

        if let ProtocolConfig::Ospf {
            table,
            router_id,
            areas,
            ..
        } = &config
        {
            self.table = table.clone();
            self.router_id = router_id.clone().unwrap_or_default();
            self.areas = areas.iter().map(|a| a.area_id.clone()).collect();
        }

        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: format!(
                "OSPF started, router-id {}, {} areas",
                self.router_id,
                self.areas.len()
            ),
        });

        let hello_interval = Duration::from_secs(10);
        let mut hello_tick = interval(hello_interval);
        hello_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut spf_tick = interval(Duration::from_secs(10));
        spf_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            select! {
                msg = rx.recv() => {
                    match msg {
                        Some(ProtocolMsg::Shutdown) => {
                            return Err(ProtocolError::Stopped(self.name.clone(), "shutdown".into()));
                        }
                        Some(ProtocolMsg::Enable) => {
                            self.state = ProtocolState::Up;
                        }
                        Some(ProtocolMsg::Disable) => {
                            self.state = ProtocolState::Down;
                        }
                        Some(ProtocolMsg::StatusQuery { reply }) => {
                            let _ = reply.send(netpilot_protocol::event::ProtocolStatus {
                                name: self.name.clone(),
                                state: self.state.clone(),
                                uptime_secs: 0,
                                routes_imported: 0,
                                routes_exported: 0,
                            });
                        }
                        None => return Ok(()),
                        _ => {}
                    }
                }
                _ = hello_tick.tick() => {
                    // In a full implementation, this would send Hello packets
                    // via the OspfTransport on each configured interface.
                    // For now, we just maintain the tick for timing.
                }
                _ = spf_tick.tick() => {
                    let routes = crate::spf::compute_ospf_spf(&self.lsdb, &self.router_id);
                    for route in &routes {
                        self.emit(ProtocolEvent::RouteAnnounce {
                            table: self.table.clone(),
                            prefix: route.prefix.clone(),
                            next_hop: route.next_hop.clone(),
                            preference: 110,
                            source_protocol: "ospf".into(),
                            attributes: RouteAttributes {
                                metric: Some(route.cost),
                                ..Default::default()
                            },
                        });
                    }
                }
            }
        }
    }
}
