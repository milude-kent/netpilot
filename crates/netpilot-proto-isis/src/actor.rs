use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use std::collections::HashMap;
use tokio::select;
use tokio::sync::mpsc;

use crate::adjacency::Adjacency;
use crate::config::{IsisConfig, IsisLevel};
use crate::lsp::LspDatabase;
use crate::packet::{IihPacket, IsisPacket, IsisPacketBody, PduType};
use crate::spf::compute_spf;
use crate::timer::IsisTimers;
use crate::transport::IsisTransport;

pub struct IsisActor {
    name: String,
    config: IsisConfig,
    adjacencies: HashMap<String, Adjacency>,
    lsp_db: LspDatabase,
    timers: IsisTimers,
    transport: Option<Box<dyn IsisTransport>>,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
    sequence_number: u32,
    state: ProtocolState,
    stats: ProtocolStats,
}

impl IsisActor {
    pub fn new(config: IsisConfig) -> Self {
        Self {
            name: config.name.clone(),
            config,
            adjacencies: HashMap::new(),
            lsp_db: LspDatabase::new(),
            timers: IsisTimers::default_timers(),
            transport: None,
            event_tx: None,
            sequence_number: 1,
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
        }
    }

    /// Swap in a real transport (e.g. `LoopbackTransport` for testing,
    /// or a future raw socket transport for Linux).
    pub fn set_transport(&mut self, transport: Box<dyn IsisTransport>) {
        self.transport = Some(transport);
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    async fn handle_msg(&mut self, msg: ProtocolMsg) -> Result<(), ProtocolError> {
        match msg {
            ProtocolMsg::Reload { config, scope } => {
                if let ProtocolConfig::Isis {
                    interfaces,
                    levels,
                    area_addresses,
                    system_id,
                    ..
                } = config
                {
                    self.config.interfaces = interfaces.into_iter().map(Into::into).collect();
                    self.config.levels = levels.into_iter().map(Into::into).collect();
                    self.config.area_addresses = area_addresses;
                    self.config.system_id = system_id;
                }
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: self.state.clone(),
                    message: format!("config reloaded ({:?})", scope),
                });
                Ok(())
            }
            ProtocolMsg::Enable => {
                self.state = ProtocolState::Up;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Up,
                    message: "protocol enabled".into(),
                });
                Ok(())
            }
            ProtocolMsg::Disable => {
                self.state = ProtocolState::Down;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Down,
                    message: "protocol disabled".into(),
                });
                Ok(())
            }
            ProtocolMsg::Restart => {
                self.state = ProtocolState::Start;
                self.adjacencies.clear();
                self.lsp_db = LspDatabase::new();
                self.sequence_number = 1;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Start,
                    message: "protocol restarting".into(),
                });
                Ok(())
            }
            ProtocolMsg::Shutdown => {
                self.state = ProtocolState::Down;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Down,
                    message: "protocol shutting down".into(),
                });
                Err(ProtocolError::Stopped(
                    self.name.clone(),
                    "shutdown requested".into(),
                ))
            }
            ProtocolMsg::StatusQuery { reply } => {
                let status = netpilot_protocol::event::ProtocolStatus {
                    name: self.name.clone(),
                    state: self.state.clone(),
                    uptime_secs: 0,
                    routes_imported: self.stats.routes_imported,
                    routes_exported: self.stats.routes_exported,
                };
                let _ = reply.send(status);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn handle_packet(&mut self, iface: &str, pkt: &IsisPacket) {
        match &pkt.body {
            IsisPacketBody::Iih(iih) => {
                let neighbor_id = &iih.source_id;
                let key = format!("{}/{}", iface, neighbor_id);
                let adj = self.adjacencies.entry(key).or_insert_with(|| {
                    Adjacency::new(
                        neighbor_id,
                        iface,
                        IsisLevel::Level2,
                        &self.config.system_id,
                        30,
                    )
                });
                let old_state = adj.state.clone();
                let new_state = adj.process_hello(iih);
                if new_state != old_state {
                    self.emit(ProtocolEvent::StateChange {
                        protocol_name: self.name.clone(),
                        new_state: self.state.clone(),
                        message: format!(
                            "adjacency {}/{} {:?} -> {:?}",
                            iface, neighbor_id, old_state, new_state
                        ),
                    });
                }
                self.stats.updates_received += 1;
            }
            IsisPacketBody::Lsp(lsp) => {
                if !self.lsp_db.contains_newer(&lsp.lsp_id, lsp.sequence_number) {
                    let now = time::OffsetDateTime::now_utc();
                    let expires = now + time::Duration::seconds(lsp.remaining_lifetime_secs as i64);
                    self.lsp_db.insert(crate::lsp::LspEntry {
                        lsp_id: lsp.lsp_id.clone(),
                        sequence_number: lsp.sequence_number,
                        remaining_lifetime_secs: lsp.remaining_lifetime_secs,
                        checksum: lsp.checksum,
                        tlvs: lsp.tlvs.clone(),
                        received_at: now,
                        expires_at: expires,
                    });
                    self.stats.updates_received += 1;
                }
            }
            IsisPacketBody::Csnp(_csnp) => {
                // Full CSNP processing would trigger LSP synchronization
                self.stats.updates_received += 1;
            }
            IsisPacketBody::Psnp(_psnp) => {
                // PSNP processing would request missing LSPs
                self.stats.updates_received += 1;
            }
        }
    }

    async fn hello_tick(&mut self) {
        let iih = IihPacket {
            circuit_type: 3,
            source_id: self.config.system_id.clone(),
            holding_time_secs: 30,
            pdu_length: 100,
            priority: 64,
            lan_id: None,
            neighbors: self
                .adjacencies
                .values()
                .filter(|a| a.is_up())
                .map(|a| a.neighbor_system_id.clone())
                .collect(),
            tlvs: vec![],
        };

        let pkt = IsisPacket {
            header: crate::packet::IsisHeader {
                protocol_id: 0x83,
                header_length: 8,
                version: 1,
                system_id_length: 0,
                pdu_type: PduType::Level2LanIih,
                version2: 1,
                reserved: 0,
                max_area_addresses: 3,
            },
            body: IsisPacketBody::Iih(iih),
        };

        // Send hello on each configured interface
        if let Some(ref transport) = self.transport {
            for iface in &self.config.interfaces {
                let _ = transport.send(&iface.interface, &pkt).await;
            }
        }
    }

    async fn spf_tick(&mut self) {
        if self.adjacencies.values().any(|a| a.is_up()) {
            let result = compute_spf(&self.lsp_db, &self.config.system_id);
            for route in &result.routes {
                self.emit(ProtocolEvent::RouteAnnounce {
                    table: self.config.table.clone(),
                    prefix: route.prefix.clone(),
                    next_hop: route.next_hop.clone(),
                    preference: 115,
                    attributes: RouteAttributes {
                        metric: Some(route.metric),
                        ..Default::default()
                    },
                });
                self.stats.routes_exported += 1;
            }
        }
    }
}

#[async_trait]
impl ProtocolActor for IsisActor {
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

        // Extract IsisConfig from ProtocolConfig
        if let ProtocolConfig::Isis {
            table,
            area_addresses,
            system_id,
            levels,
            interfaces,
            sr_enabled,
            ..
        } = &config
        {
            self.config.table = table.clone();
            self.config.area_addresses = area_addresses.clone();
            self.config.system_id = system_id.clone();
            self.config.levels = levels.clone().into_iter().map(Into::into).collect();
            self.config.interfaces = interfaces.clone().into_iter().map(Into::into).collect();
            self.config.sr_enabled = *sr_enabled;
        }

        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: "IS-IS protocol started".into(),
        });

        loop {
            select! {
                msg = rx.recv() => {
                    match msg {
                        Some(m) => {
                            if let Err(e) = self.handle_msg(m).await {
                                // Shutdown returns Stopped error to break the loop
                                self.emit(ProtocolEvent::Error {
                                    protocol_name: self.name.clone(),
                                    message: e.to_string(),
                                });
                                return Err(e);
                            }
                        }
                        None => {
                            return Ok(()); // channel closed
                        }
                    }
                }
                recv_result = async {
                    if let Some(ref mut transport) = self.transport {
                        transport.recv().await
                    } else {
                        std::future::pending::<Result<(String, IsisPacket), crate::transport::TransportError>>().await
                    }
                } => {
                    match recv_result {
                        Ok((iface, pkt)) => {
                            self.handle_packet(&iface, &pkt).await;
                        }
                        Err(e) => {
                            self.emit(ProtocolEvent::Error {
                                protocol_name: self.name.clone(),
                                message: format!("transport recv error: {e}"),
                            });
                        }
                    }
                }
                _ = self.timers.hello_interval.tick() => {
                    self.hello_tick().await;
                }
                _ = self.timers.hold_check_interval.tick() => {
                    let mut expired_neighbors = Vec::new();
                    for adj in self.adjacencies.values_mut() {
                        let expired = adj.tick_holding_timer();
                        if expired {
                            expired_neighbors.push(adj.neighbor_system_id.clone());
                        }
                    }
                    for neighbor_id in expired_neighbors {
                        self.emit(ProtocolEvent::StateChange {
                            protocol_name: self.name.clone(),
                            new_state: self.state.clone(),
                            message: format!("adjacency {} expired", neighbor_id),
                        });
                    }
                }
                _ = self.timers.lsp_refresh_interval.tick() => {
                    let self_lsp = self.lsp_db.generate_self_lsp(
                        &self.config.system_id,
                        &self.adjacencies.values().cloned().collect::<Vec<_>>(),
                    );
                    self.emit(ProtocolEvent::Error {
                        protocol_name: self.name.clone(),
                        message: format!("LSP refresh: {} seq={}", self_lsp.lsp_id.display(), self_lsp.sequence_number),
                    });
                }
                _ = self.timers.spf_interval.tick() => {
                    self.spf_tick().await;
                }
                _ = self.timers.purge_interval.tick() => {
                    let purged = self.lsp_db.purge_expired();
                    if purged > 0 {
                        self.emit(ProtocolEvent::Error {
                            protocol_name: self.name.clone(),
                            message: format!("purged {} expired LSPs", purged),
                        });
                    }
                }
            }
        }
    }
}
