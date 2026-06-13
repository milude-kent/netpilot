use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats};
use tokio::sync::mpsc;
use tokio::select;
use tokio::time::{interval, Duration, MissedTickBehavior};

#[derive(Clone, Debug)]
pub struct MulticastGroup {
    pub group: String,
    pub source: String,
    pub upstream_interface: String,
    pub downstream_interfaces: Vec<String>,
}

pub struct PimActor {
    name: String,
    router_id: String,
    state: ProtocolState,
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
    pub groups: Vec<MulticastGroup>,
}

impl PimActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            router_id: String::new(),
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            event_tx: None,
            groups: vec![],
        }
    }

    pub fn with_event_tx(mut self, tx: tokio::sync::broadcast::Sender<ProtocolEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    pub fn join_group(&mut self, group: &str, source: &str) {
        self.groups.push(MulticastGroup {
            group: group.to_string(),
            source: source.to_string(),
            upstream_interface: "eth0".into(),
            downstream_interfaces: vec![],
        });
        self.stats.routes_imported += 1;
    }

    pub fn leave_group(&mut self, group: &str) {
        self.groups.retain(|g| g.group != group);
    }
}

#[async_trait]
impl ProtocolActor for PimActor {
    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name;
        if let ProtocolConfig::Pim { router_id, .. } = &config {
            self.router_id = router_id.clone();
        }
        self.state = ProtocolState::Start;
        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: format!("PIM started (router-id: {})", self.router_id),
        });

        let mut hello_tick = interval(Duration::from_secs(30));
        hello_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

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
                _ = hello_tick.tick() => {
                    // Send PIM Hello messages
                }
            }
        }
    }
}
