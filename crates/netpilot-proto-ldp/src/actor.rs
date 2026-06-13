use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats};
use tokio::sync::mpsc;
use tokio::select;
use tokio::time::{interval, Duration, MissedTickBehavior};

pub struct LdpActor {
    name: String,
    lsr_id: String,
    state: ProtocolState,
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
}

impl LdpActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            lsr_id: String::new(),
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            event_tx: None,
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
                                routes_imported: 0,
                                routes_exported: 0,
                            });
                        }
                        None => return Ok(()),
                        _ => {}
                    }
                }
                _ = hello_tick.tick() => {
                    // Send LDP Hello messages
                }
            }
        }
    }
}
