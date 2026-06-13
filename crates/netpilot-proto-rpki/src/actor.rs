use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats};
use netpilot_protocol::{ProtocolActor, ProtocolError, ProtocolMsg};
use std::collections::HashMap;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, MissedTickBehavior};

pub struct RpkiActor {
    name: String,
    state: ProtocolState,
    roas: HashMap<String, Vec<u32>>, // prefix → allowed ASNs
    aspas: HashMap<u32, Vec<u32>>,   // customer AS → provider AS set
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
}

impl RpkiActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            state: ProtocolState::Down,
            roas: HashMap::new(),
            aspas: HashMap::new(),
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

    /// Check ROA validity: does the prefix have the ASN in its allowed list?
    pub fn validate_roa(&self, prefix: &str, asn: u32) -> RoAStatus {
        if let Some(allowed) = self.roas.get(prefix) {
            if allowed.contains(&asn) {
                RoAStatus::Valid
            } else {
                RoAStatus::Invalid
            }
        } else {
            RoAStatus::NotFound
        }
    }

    /// Check ASPA validity for a customer/provider pair.
    pub fn validate_aspa(&self, customer_as: u32, provider_as: u32) -> AspaStatus {
        if let Some(providers) = self.aspas.get(&customer_as) {
            if providers.contains(&provider_as) {
                AspaStatus::Valid
            } else {
                AspaStatus::Invalid
            }
        } else {
            AspaStatus::Unknown
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoAStatus {
    Valid,
    Invalid,
    NotFound,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AspaStatus {
    Valid,
    Invalid,
    Unknown,
}

#[async_trait]
impl ProtocolActor for RpkiActor {
    async fn run(
        &mut self,
        name: String,
        _config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name;
        self.state = ProtocolState::Start;
        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: "RPKI started".into(),
        });

        let mut refresh_tick = interval(Duration::from_secs(3600));
        refresh_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

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
                _ = refresh_tick.tick() => {
                    // Connect to RTR cache, refresh ROA/ASPA data
                }
            }
        }
    }
}
