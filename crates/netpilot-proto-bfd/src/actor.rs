use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState};
use netpilot_protocol::{ProtocolActor, ProtocolError, ProtocolMsg};
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{Duration, MissedTickBehavior, interval};

pub struct BfdActor {
    name: String,
    state: ProtocolState,
    sessions: Vec<BfdSession>,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
}

#[derive(Clone, Debug)]
pub struct BfdSession {
    pub local_discriminator: u32,
    pub remote_discriminator: Option<u32>,
    pub desired_min_tx_ms: u32,
    pub required_min_rx_ms: u32,
    pub multiplier: u8,
    pub state: BfdSessionState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BfdSessionState {
    AdminDown,
    Down,
    Init,
    Up,
}

impl Default for BfdActor {
    fn default() -> Self {
        Self::new()
    }
}

impl BfdActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            state: ProtocolState::Down,
            sessions: vec![],
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
impl ProtocolActor for BfdActor {
    fn set_event_tx(&mut self, tx: tokio::sync::broadcast::Sender<ProtocolEvent>) {
        self.event_tx = Some(tx);
    }

    async fn run(
        &mut self,
        name: String,
        _config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name;
        self.state = ProtocolState::Start;

        // Default BFD session for testing
        self.sessions.push(BfdSession {
            local_discriminator: 1,
            remote_discriminator: None,
            desired_min_tx_ms: 300,
            required_min_rx_ms: 300,
            multiplier: 3,
            state: BfdSessionState::Down,
        });

        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: "BFD started".into(),
        });

        let mut bfd_tick = interval(Duration::from_millis(300));
        bfd_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            select! {
                msg = rx.recv() => {
                    match msg {
                        Some(ProtocolMsg::Shutdown) => {
                            return Err(ProtocolError::Stopped(
                                self.name.clone(),
                                "shutdown".into(),
                            ));
                        }
                        Some(ProtocolMsg::Enable) => {
                            self.state = ProtocolState::Up;
                            for s in &mut self.sessions {
                                s.state = BfdSessionState::Up;
                            }
                            self.emit(ProtocolEvent::StateChange {
                                protocol_name: self.name.clone(),
                                new_state: ProtocolState::Up,
                                message: "BFD sessions up".into(),
                            });
                        }
                        Some(ProtocolMsg::Disable) => {
                            for s in &mut self.sessions {
                                s.state = BfdSessionState::AdminDown;
                            }
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
                _ = bfd_tick.tick() => {
                    // Send BFD control packets, check detection time
                }
            }
        }
    }
}
