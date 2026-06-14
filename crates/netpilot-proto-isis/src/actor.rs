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

                    // Flood the LSP to all other adjacencies (except the one it came from)
                    self.flood_lsp(lsp, iface).await;
                }
            }
            IsisPacketBody::Csnp(csnp) => {
                // Compare CSNP entries against our LSDB to find missing/newer LSPs.
                // Send PSNP requesting missing ones, and flood any LSPs we have
                // that are newer than what the CSNP indicates.
                self.process_csnp(csnp, iface).await;
                self.stats.updates_received += 1;
            }
            IsisPacketBody::Psnp(psnp) => {
                // PSNP can serve as:
                // 1. Request for missing LSPs (send them)
                // 2. Acknowledgment on P2P links (mark as received)
                self.process_psnp(psnp, iface).await;
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
                    source_protocol: "isis".into(),
                    attributes: RouteAttributes {
                        metric: Some(route.metric),
                        ..Default::default()
                    },
                });
                self.stats.routes_exported += 1;
            }
        }
    }

    /// Flood an LSP to all adjacencies except the receiving interface.
    async fn flood_lsp(&mut self, lsp: &crate::packet::LspPacket, received_iface: &str) {
        if self.transport.is_none() {
            return;
        }

        let flood_pkt = IsisPacket {
            header: crate::packet::IsisHeader {
                protocol_id: 0x83,
                header_length: 8,
                version: 1,
                system_id_length: 0,
                pdu_type: PduType::Level2Lsp,
                version2: 1,
                reserved: 0,
                max_area_addresses: 3,
            },
            body: IsisPacketBody::Lsp(lsp.clone()),
        };

        if let Some(ref transport) = self.transport {
            for iface in &self.config.interfaces {
                // Don't flood back to the interface we received the LSP on
                if iface.interface == received_iface {
                    continue;
                }
                // Only flood to interfaces that have at least one Up adjacency
                let has_up_adj = self
                    .adjacencies
                    .values()
                    .any(|a| a.interface == iface.interface && a.is_up());
                if !has_up_adj {
                    continue;
                }
                if let Err(e) = transport.send(&iface.interface, &flood_pkt).await {
                    self.emit(ProtocolEvent::Error {
                        protocol_name: self.name.clone(),
                        message: format!("LSP flood failed on {}: {}", iface.interface, e),
                    });
                }
            }
        }
    }

    /// Send an LSP packet on all interfaces with Up adjacencies.
    async fn send_lsp_packet(&mut self, lsp: &crate::packet::LspPacket) {
        if self.transport.is_none() {
            return;
        }

        let pkt = IsisPacket {
            header: crate::packet::IsisHeader {
                protocol_id: 0x83,
                header_length: 8,
                version: 1,
                system_id_length: 0,
                pdu_type: PduType::Level2Lsp,
                version2: 1,
                reserved: 0,
                max_area_addresses: 3,
            },
            body: IsisPacketBody::Lsp(lsp.clone()),
        };

        if let Some(ref transport) = self.transport {
            for iface in &self.config.interfaces {
                let has_up_adj = self
                    .adjacencies
                    .values()
                    .any(|a| a.interface == iface.interface && a.is_up());
                if has_up_adj {
                    let _ = transport.send(&iface.interface, &pkt).await;
                }
            }
        }
    }

    /// Process a received CSNP: compare entries against our LSDB.
    ///
    /// For each CSNP entry:
    /// - If we don't have it or our copy is older → send PSNP to request it
    /// - If we have a newer copy → flood our LSP to the sender
    async fn process_csnp(&mut self, csnp: &crate::packet::CsnpPacket, received_iface: &str) {
        let mut missing_lsp_ids: Vec<crate::packet::LspId> = Vec::new();

        for entry in &csnp.lsp_entries {
            match self.lsp_db.get(&entry.lsp_id) {
                None => {
                    // We don't have this LSP — request it via PSNP
                    missing_lsp_ids.push(entry.lsp_id.clone());
                }
                Some(our_entry) => {
                    if our_entry.sequence_number < entry.sequence_number {
                        // Our copy is older — request the newer version
                        missing_lsp_ids.push(entry.lsp_id.clone());
                    } else if our_entry.sequence_number > entry.sequence_number {
                        // Our copy is newer — flood it to the sender
                        let lsp_pkt = crate::packet::LspPacket {
                            pdu_length: 0,
                            lsp_id: our_entry.lsp_id.clone(),
                            sequence_number: our_entry.sequence_number,
                            remaining_lifetime_secs: our_entry.remaining_lifetime_secs,
                            checksum: our_entry.checksum,
                            flags: crate::packet::LspFlags::default(),
                            tlvs: our_entry.tlvs.clone(),
                        };
                        self.flood_lsp(&lsp_pkt, received_iface).await;
                    }
                    // Same sequence number — nothing to do
                }
            }
        }

        // Send PSNP requesting missing LSPs
        if !missing_lsp_ids.is_empty() {
            self.send_psnp(&missing_lsp_ids, received_iface).await;
        }
    }

    /// Process a received PSNP: send requested LSPs.
    async fn process_psnp(&mut self, psnp: &crate::packet::PsnpPacket, _received_iface: &str) {
        for entry in &psnp.lsp_entries {
            if let Some(our_entry) = self.lsp_db.get(&entry.lsp_id) {
                let lsp_pkt = crate::packet::LspPacket {
                    pdu_length: 0,
                    lsp_id: our_entry.lsp_id.clone(),
                    sequence_number: our_entry.sequence_number,
                    remaining_lifetime_secs: our_entry.remaining_lifetime_secs,
                    checksum: our_entry.checksum,
                    flags: crate::packet::LspFlags::default(),
                    tlvs: our_entry.tlvs.clone(),
                };
                self.send_lsp_packet(&lsp_pkt).await;
            }
        }
    }

    /// Send a PSNP requesting specific LSPs on a given interface.
    async fn send_psnp(&mut self, lsp_ids: &[crate::packet::LspId], iface: &str) {
        if self.transport.is_none() {
            return;
        }

        let lsp_entries: Vec<crate::packet::CsnpLspEntry> = lsp_ids
            .iter()
            .map(|id| crate::packet::CsnpLspEntry {
                lsp_id: id.clone(),
                sequence_number: 0, // we don't know the seq — request any
                remaining_lifetime_secs: 0,
                checksum: 0,
            })
            .collect();

        let psnp = crate::packet::PsnpPacket {
            pdu_length: 0,
            source_id: self.config.system_id.clone(),
            lsp_entries,
            tlvs: vec![],
        };

        let pkt = IsisPacket {
            header: crate::packet::IsisHeader {
                protocol_id: 0x83,
                header_length: 8,
                version: 1,
                system_id_length: 0,
                pdu_type: PduType::Level2Psnp,
                version2: 1,
                reserved: 0,
                max_area_addresses: 3,
            },
            body: IsisPacketBody::Psnp(psnp),
        };

        if let Some(ref transport) = self.transport {
            let _ = transport.send(iface, &pkt).await;
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
                    tracing::info!(
                        lsp_id = %self_lsp.lsp_id.display(),
                        seq = self_lsp.sequence_number,
                        "IS-IS LSP refresh"
                    );
                    // Also insert into our own LSDB
                    let now = time::OffsetDateTime::now_utc();
                    let expires = now + time::Duration::seconds(self_lsp.remaining_lifetime_secs as i64);
                    self.lsp_db.insert(crate::lsp::LspEntry {
                        lsp_id: self_lsp.lsp_id.clone(),
                        sequence_number: self_lsp.sequence_number,
                        remaining_lifetime_secs: self_lsp.remaining_lifetime_secs,
                        checksum: self_lsp.checksum,
                        tlvs: self_lsp.tlvs.clone(),
                        received_at: now,
                        expires_at: expires,
                    });
                    // Send on all interfaces with Up adjacencies
                    self.send_lsp_packet(&self_lsp).await;
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
