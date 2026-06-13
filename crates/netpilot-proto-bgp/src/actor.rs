use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats};
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use tokio::select;
use tokio::sync::mpsc;

#[allow(dead_code)]
pub struct BgpActor {
    name: String,
    table: String,
    local_asn: u32,
    neighbors: Vec<(String, u32, String)>, // (addr, asn, name)
    state: ProtocolState,
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
}

impl BgpActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            table: String::new(),
            local_asn: 0,
            neighbors: vec![],
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
impl ProtocolActor for BgpActor {
    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name;
        self.state = ProtocolState::Start;

        if let ProtocolConfig::Bgp {
            table,
            local_asn,
            neighbors,
            ..
        } = &config
        {
            self.table = table.clone();
            self.local_asn = *local_asn;
            self.neighbors = neighbors
                .iter()
                .map(|n| {
                    (
                        n.remote_address.clone(),
                        n.remote_asn,
                        n.name.clone(),
                    )
                })
                .collect();
        }

        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: format!("BGP started with {} peers", self.neighbors.len()),
        });

        // For each neighbor, attempt TCP connection (non-blocking).
        // Collect all required data into owned values before spawning.
        let spawn_neighbors: Vec<_> = self
            .neighbors
            .iter()
            .map(|(addr, asn, neighbor_name)| {
                (
                    addr.clone(),
                    *asn,
                    neighbor_name.clone(),
                    self.name.clone(),
                    self.event_tx.clone(),
                )
            })
            .collect();

        for (addr, asn, neighbor_name, protocol_name, event_tx) in spawn_neighbors {
            tokio::spawn(async move {
                let mut session = netpilot_io::bgp::BgpSession::new(&addr, 0, asn);
                match session.connect().await {
                    Ok(()) => {
                        if let Some(tx) = &event_tx {
                            let _ = tx.send(ProtocolEvent::StateChange {
                                protocol_name: protocol_name.clone(),
                                new_state: ProtocolState::Up,
                                message: format!("BGP peer {} established", neighbor_name),
                            });
                        }
                    }
                    Err(e) => {
                        if let Some(tx) = &event_tx {
                            let _ = tx.send(ProtocolEvent::Error {
                                protocol_name: protocol_name.clone(),
                                message: format!("BGP peer {} failed: {}", neighbor_name, e),
                            });
                        }
                    }
                }
            });
        }

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
            }
        }
    }
}
