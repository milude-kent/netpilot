use async_trait::async_trait;
use std::collections::HashMap;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
use tokio::sync::mpsc;
use tokio::select;
use tokio::time::{interval, Duration, MissedTickBehavior};

pub struct LdpActor {
    name: String,
    lsr_id: String,
    state: ProtocolState,
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
    pub label_bindings: HashMap<String, u32>, // prefix -> label
}

impl LdpActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            lsr_id: String::new(),
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            event_tx: None,
            label_bindings: HashMap::new(),
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

    /// Allocate a label for a prefix from the MPLS label pool.
    pub fn bind_label(&mut self, prefix: &str, label: u32) {
        self.label_bindings.insert(prefix.to_string(), label);
        self.stats.routes_exported += 1;
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(ProtocolEvent::RouteAnnounce {
                table: "mpls".into(),
                prefix: prefix.to_string(),
                next_hop: self.lsr_id.clone(),
                preference: 150,
                attributes: Default::default(),
            });
        }
    }

    /// Withdraw a label binding.
    pub fn withdraw_label(&mut self, prefix: &str) {
        self.label_bindings.remove(prefix);
    }

    /// Periodic label distribution: send label mappings to peers.
    pub fn distribute_labels(&self) -> Vec<(&str, u32)> {
        self.label_bindings.iter().map(|(p, l)| (p.as_str(), *l)).collect()
    }
}

#[async_trait]
impl ProtocolActor for LdpActor {
    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name;
        if let ProtocolConfig::Ldp { lsr_id, .. } = &config {
            self.lsr_id = lsr_id.clone();
        }
        self.state = ProtocolState::Start;
        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: format!("LDP started (lsr-id: {})", self.lsr_id),
        });

        let mut hello_tick = interval(Duration::from_secs(5));
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
                    // Send LDP Hello messages
                    // Also distribute label bindings
                    for (prefix, label) in self.distribute_labels() {
                        self.emit(ProtocolEvent::RouteAnnounce {
                            table: "mpls".into(),
                            prefix: prefix.to_string(),
                            next_hop: self.lsr_id.clone(),
                            preference: 150,
                            attributes: RouteAttributes { mpls_label: Some(label), ..Default::default() },
                        });
                    }
                }
            }
        }
    }
}
