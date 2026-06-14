use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use std::collections::HashMap;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{Duration, MissedTickBehavior, interval};

pub struct RipActor {
    name: String,
    router_id: String,
    state: ProtocolState,
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
    pub routing_table: HashMap<String, u32>,
}

impl Default for RipActor {
    fn default() -> Self {
        Self::new()
    }
}

impl RipActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            router_id: String::new(),
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            event_tx: None,
            routing_table: HashMap::new(),
        }
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Run distance-vector update: send our routing table to neighbors.
    pub fn run_distance_vector(&mut self, routes: &HashMap<String, u32>) {
        for (prefix, metric) in routes {
            // RIP max metric is 15, 16 = infinity/unreachable
            let new_metric = (metric + 1).min(16);
            self.routing_table.insert(prefix.clone(), new_metric);

            if new_metric < 16
                && let Some(ref tx) = self.event_tx
            {
                let _ = tx.send(ProtocolEvent::RouteAnnounce {
                    table: "rip".into(),
                    prefix: prefix.clone(),
                    next_hop: self.router_id.clone(),
                    preference: 120,
                    source_protocol: "rip".into(),
                    attributes: RouteAttributes {
                        metric: Some(new_metric),
                        ..Default::default()
                    },
                });
            }
        }
    }
}

#[async_trait]
impl ProtocolActor for RipActor {
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
        if let ProtocolConfig::Rip { router_id, .. } = &config {
            self.router_id = router_id.clone();
        }
        self.state = ProtocolState::Start;
        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: format!("RIP started (router-id: {})", self.router_id),
        });

        let mut update_tick = interval(Duration::from_secs(30));
        update_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

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
                                routes_imported: self.stats.routes_imported,
                                routes_exported: self.stats.routes_exported,
                            });
                        }
                        None => return Ok(()),
                        _ => {}
                    }
                }
                _ = update_tick.tick() => {
                    // Send RIP update messages
                }
            }
        }
    }
}
