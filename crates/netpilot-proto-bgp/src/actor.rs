use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
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

/// Parse a 2-byte ASN AS_PATH path attribute value.
/// Format: sequence of (segment_type: u8, segment_length: u8, ASN1[2], ASN2[2], ...)
/// Extract route attributes from raw BGP path attributes.
///
/// Returns (RouteAttributes, Option<next_hop>) where next_hop is
/// parsed from BGP attribute code 3 (NEXT_HOP).
fn extract_route_attributes(
    attrs: &[netpilot_io::bgp::BgpAttribute],
) -> (RouteAttributes, Option<String>) {
    let mut ra = RouteAttributes::default();
    let mut next_hop: Option<String> = None;

    for attr in attrs {
        match attr.code {
            2 => {
                // AS_PATH (2-byte ASNs, RFC 4271)
                let asns = parse_as_path_2byte(&attr.value);
                if !asns.is_empty() {
                    ra.as_path = Some(asns);
                }
            }
            3 if attr.value.len() == 4 => {
                // NEXT_HOP
                next_hop = Some(format!(
                    "{}.{}.{}.{}",
                    attr.value[0], attr.value[1], attr.value[2], attr.value[3]
                ));
            }
            4 if attr.value.len() == 4 => {
                // MULTI_EXIT_DISC (MED)
                ra.metric = Some(u32::from_be_bytes([
                    attr.value[0],
                    attr.value[1],
                    attr.value[2],
                    attr.value[3],
                ]));
            }
            5 if attr.value.len() == 4 => {
                // LOCAL_PREF
                ra.local_pref = Some(u32::from_be_bytes([
                    attr.value[0],
                    attr.value[1],
                    attr.value[2],
                    attr.value[3],
                ]));
            }
            8 => {
                // COMMUNITY (RFC 1997)
                let mut comms = Vec::new();
                let mut pos = 0;
                while pos + 4 <= attr.value.len() {
                    let high = u16::from_be_bytes([attr.value[pos], attr.value[pos + 1]]);
                    let low = u16::from_be_bytes([attr.value[pos + 2], attr.value[pos + 3]]);
                    comms.push(format!("{high}:{low}"));
                    pos += 4;
                }
                if !comms.is_empty() {
                    ra.communities = Some(comms);
                }
            }
            17 => {
                // AS4_PATH (4-byte ASNs, RFC 6793)
                // AS4_PATH takes precedence over AS_PATH per RFC 6793 §6
                let asns = parse_as_path_4byte(&attr.value);
                if !asns.is_empty() {
                    ra.as_path = Some(asns);
                }
            }
            _ => {} // Ignore unrecognized optional attributes
        }
    }
    (ra, next_hop)
}

fn parse_as_path_2byte(data: &[u8]) -> Vec<u32> {
    let mut asns = Vec::new();
    let mut pos = 0;
    while pos + 2 <= data.len() {
        let _seg_type = data[pos]; // 1=AS_SET, 2=AS_SEQUENCE
        let seg_len = data[pos + 1] as usize;
        pos += 2;
        for _ in 0..seg_len {
            if pos + 2 <= data.len() {
                let asn = u32::from_be_bytes([0, 0, data[pos], data[pos + 1]]);
                asns.push(asn);
                pos += 2;
            } else {
                break;
            }
        }
    }
    asns
}

/// Parse a 4-byte ASN AS4_PATH path attribute value (RFC 6793).
/// Same format as AS_PATH but each ASN is 4 bytes.
fn parse_as_path_4byte(data: &[u8]) -> Vec<u32> {
    let mut asns = Vec::new();
    let mut pos = 0;
    while pos + 2 <= data.len() {
        let _seg_type = data[pos];
        let seg_len = data[pos + 1] as usize;
        pos += 2;
        for _ in 0..seg_len {
            if pos + 4 <= data.len() {
                let asn =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                asns.push(asn);
                pos += 4;
            } else {
                break;
            }
        }
    }
    asns
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
                    self.local_asn,
                )
            })
            .collect();

        for (addr, asn, neighbor_name, name, tx, local_asn) in spawn_neighbors {
            let handle = tokio::spawn(async move {
                let mut retry_delay = 5u64; // start at 5s, grow to 300s
                let mut consecutive_failures = 0u32;
                loop {
                    let mut session = netpilot_io::bgp::BgpSession::new(&addr, local_asn, asn);
                    match session.connect().await {
                        Ok(()) => {
                            consecutive_failures = 0;
                            retry_delay = 5;
                            if let Some(ref tx) = tx {
                                let _ = tx.send(ProtocolEvent::StateChange {
                                    protocol_name: name.clone(),
                                    new_state: ProtocolState::Up,
                                    message: format!("BGP peer {} established", neighbor_name),
                                });
                            }

                            // Compute keepalive and hold timers from negotiated values
                            let hold_secs = session.hold_time_secs as u64;
                            let keepalive_secs = (hold_secs / 3).clamp(1, 60);
                            let hold_timer = tokio::time::Duration::from_secs(hold_secs);
                            let keepalive_interval =
                                tokio::time::Duration::from_secs(keepalive_secs);

                            // Keepalive + receive loop with hold timer
                            loop {
                                tokio::select! {
                                    _ = tokio::time::sleep(keepalive_interval) => {
                                        if session.send_keepalive().await.is_err() {
                                            break;
                                        }
                                    }
                                    result = tokio::time::timeout(hold_timer, session.recv_message()) => {
                                        match result {
                                            Ok(Ok(netpilot_io::bgp::BgpMessage::Update {
                                                nlri,
                                                path_attributes,
                                                withdrawn_routes,
                                            })) => {
                                                let (route_attrs, parsed_next_hop) =
                                                    extract_route_attributes(&path_attributes);
                                                let effective_next_hop =
                                                    parsed_next_hop.unwrap_or_else(|| addr.clone());

                                                // Emit RouteWithdraw for withdrawn routes
                                                for prefix in &withdrawn_routes {
                                                    if let Some(ref tx) = tx {
                                                        let _ = tx.send(ProtocolEvent::RouteWithdraw {
                                                            table: "master".into(),
                                                            prefix: prefix.clone(),
                                                        });
                                                    }
                                                }

                                                // Emit RouteAnnounce for NLRI
                                                for prefix in &nlri {
                                                    if let Some(ref tx) = tx {
                                                        let _ = tx.send(ProtocolEvent::RouteAnnounce {
                                                            table: "master".into(),
                                                            prefix: prefix.clone(),
                                                            next_hop: effective_next_hop.clone(),
                                                            preference: 100,
                                                            source_protocol: "bgp".into(),
                                                            attributes: route_attrs.clone(),
                                                        });
                                                    }
                                                }
                                            }
                                            Ok(Ok(netpilot_io::bgp::BgpMessage::Keepalive)) => {
                                                // Hold timer is implicitly reset by the select! re-entry
                                            }
                                            Ok(Ok(netpilot_io::bgp::BgpMessage::Notification {
                                                error_code,
                                                error_subcode,
                                                ..
                                            })) => {
                                                if let Some(ref tx) = tx {
                                                    let _ = tx.send(ProtocolEvent::Error {
                                                        protocol_name: name.clone(),
                                                        message: format!(
                                                            "BGP NOTIFICATION from {}: code {} sub {}",
                                                            neighbor_name, error_code, error_subcode
                                                        ),
                                                    });
                                                }
                                                break;
                                            }
                                            Ok(Ok(_)) => {} // OPEN or unexpected
                                            Ok(Err(e)) => {
                                                if let Some(ref tx) = tx {
                                                    let _ = tx.send(ProtocolEvent::Error {
                                                        protocol_name: name.clone(),
                                                        message: format!(
                                                            "BGP recv error from {}: {}",
                                                            neighbor_name, e
                                                        ),
                                                    });
                                                }
                                                break;
                                            }
                                            Err(_) => {
                                                // Hold timer expired
                                                if let Some(ref tx) = tx {
                                                    let _ = tx.send(ProtocolEvent::Error {
                                                        protocol_name: name.clone(),
                                                        message: format!(
                                                            "BGP hold timer expired for {}",
                                                            neighbor_name
                                                        ),
                                                    });
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            // Session ended — emit state change
                            if let Some(ref tx) = tx {
                                let _ = tx.send(ProtocolEvent::StateChange {
                                    protocol_name: name.clone(),
                                    new_state: ProtocolState::Down,
                                    message: format!("BGP peer {} session ended", neighbor_name),
                                });
                            }
                        }
                        Err(e) => {
                            consecutive_failures += 1;
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

                    // Circuit breaker: after 10 consecutive failures, slow down significantly
                    if consecutive_failures >= 10 {
                        retry_delay = 300;
                    }

                    // Exponential backoff with cap
                    tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay)).await;
                    retry_delay = (retry_delay * 2).min(300);
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
