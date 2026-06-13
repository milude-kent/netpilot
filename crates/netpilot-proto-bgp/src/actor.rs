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
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl Default for BgpActor {
    fn default() -> Self {
        Self::new()
    }
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
            handles: vec![],
        }
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }
}

#[async_trait]
impl ProtocolActor for BgpActor {
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
                .map(|n| (n.remote_address.clone(), n.remote_asn, n.name.clone()))
                .collect();
        }

        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: format!("BGP started with {} peers", self.neighbors.len()),
        });

        // For each neighbor, spawn a persistent BGP session task.
        // Each task connects, then stays in a keepalive + receive loop,
        // reconnecting with exponential backoff on failure.
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

        for (addr, asn, neighbor_name, name, tx) in spawn_neighbors {
            let handle = tokio::spawn(async move {
                let mut retry_delay = 30u64;
                loop {
                    let mut session = netpilot_io::bgp::BgpSession::new(&addr, 0, asn);
                    match session.connect().await {
                        Ok(()) => {
                            if let Some(ref tx) = tx {
                                let _ = tx.send(ProtocolEvent::StateChange {
                                    protocol_name: name.clone(),
                                    new_state: ProtocolState::Up,
                                    message: format!("BGP peer {} established", neighbor_name),
                                });
                            }
                            // Keepalive + receive loop
                            loop {
                                tokio::select! {
                                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {
                                        // Send KEEPALIVE
                                        if session.send_keepalive().await.is_err() { break; }
                                    }
                                    result = session.recv_message() => {
                                        match result {
                                            Ok(netpilot_io::bgp::BgpMessage::Keepalive) => {}
                                            Ok(netpilot_io::bgp::BgpMessage::Update { nlri, .. }) => {
                                                for prefix in &nlri {
                                                    if let Some(ref tx) = tx {
                                                        let _ = tx.send(ProtocolEvent::RouteAnnounce {
                                                            table: "master".into(),
                                                            prefix: prefix.clone(),
                                                            next_hop: addr.clone(),
                                                            preference: 100,
                                                            attributes: Default::default(),
                                                        });
                                                    }
                                                }
                                            }
                                            Ok(_) => {}
                                            Err(_) => break,
                                        }
                                    }
                                }
                            }
                            retry_delay = 30; // reset on successful connection then break
                        }
                        Err(e) => {
                            if let Some(ref tx) = tx {
                                let _ = tx.send(ProtocolEvent::Error {
                                    protocol_name: name.clone(),
                                    message: format!(
                                        "BGP peer {} failed: {}, retry in {}s",
                                        neighbor_name, e, retry_delay
                                    ),
                                });
                            }
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay)).await;
                    retry_delay = (retry_delay * 2).min(300); // exponential backoff, max 5 min
                }
            });
            self.handles.push(handle);
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
