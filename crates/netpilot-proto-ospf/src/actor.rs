use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{Duration, MissedTickBehavior, interval};

use crate::lsdb::Lsdb;

#[allow(dead_code)]
pub struct OspfActor {
    name: String,
    table: String,
    router_id: String,
    areas: Vec<String>,
    lsdb: Lsdb,
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
            message: "OSPF started".into(),
        });

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
                _ = spf_tick.tick() => {
                    let routes = crate::spf::compute_ospf_spf(&self.lsdb, &self.router_id);
                    for route in &routes {
                        self.emit(ProtocolEvent::RouteAnnounce {
                            table: self.table.clone(),
                            prefix: route.prefix.clone(),
                            next_hop: route.next_hop.clone(),
                            preference: 110,
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
