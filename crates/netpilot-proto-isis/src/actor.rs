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

/// Per-interface IS-IS state (DIS tracking, circuit ID).
#[derive(Clone, Debug)]
pub struct IsisInterfaceState {
    pub name: String,
    pub circuit_id: u32,
    pub dis_system_id: Option<String>,
    pub dis_priority: u8,
    pub is_dis: bool,
}

impl IsisInterfaceState {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            circuit_id: 0,
            dis_system_id: None,
            dis_priority: 0,
            is_dis: false,
        }
    }

    /// Run DIS election on this interface per RFC 10589 §8.4.2.
    /// Returns true if the DIS changed.
    pub fn elect_dis(
        &mut self,
        local_system_id: &str,
        local_priority: u8,
        adjacencies: &[&Adjacency],
    ) -> bool {
        // Build candidate list: (priority, system_id) — higher priority wins,
        // ties broken by higher system ID
        let mut best_priority = local_priority;
        let mut best_id = local_system_id.to_string();

        for adj in adjacencies {
            if adj.is_up()
                && (adj.neighbor_priority > best_priority
                    || (adj.neighbor_priority == best_priority && adj.neighbor_system_id > best_id))
            {
                best_priority = adj.neighbor_priority;
                best_id = adj.neighbor_system_id.clone();
            }
        }

        let new_is_dis = best_id == local_system_id;
        let changed = self.is_dis != new_is_dis || self.dis_system_id.as_ref() != Some(&best_id);

        if changed {
            tracing::info!(
                interface = %self.name,
                old_dis = ?self.dis_system_id,
                new_dis = %best_id,
                is_dis = new_is_dis,
                "IS-IS DIS election result"
            );
        }

        self.dis_system_id = Some(best_id);
        self.dis_priority = best_priority;
        self.is_dis = new_is_dis;
        changed
    }
}

pub struct IsisActor {
    name: String,
    config: IsisConfig,
    adjacencies: HashMap<String, Adjacency>,
    lsp_db: LspDatabase,
    lsp_retrans: crate::lsp::LspRetransmissionList,
    timers: IsisTimers,
    transport: Option<Box<dyn IsisTransport>>,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
    sequence_number: u32,
    state: ProtocolState,
    stats: ProtocolStats,
    interface_states: HashMap<String, IsisInterfaceState>,
}

impl IsisActor {
    pub fn new(config: IsisConfig) -> Self {
        Self {
            name: config.name.clone(),
            config,
            adjacencies: HashMap::new(),
            lsp_db: LspDatabase::new(),
            lsp_retrans: crate::lsp::LspRetransmissionList::new(),
            timers: IsisTimers::default_timers(),
            transport: None,
            event_tx: None,
            sequence_number: 1,
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            interface_states: HashMap::new(),
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

    /// Process a received IIH (LAN or P2P) and update adjacency state.
    async fn process_iih(&mut self, iih: &crate::packet::IihPacket, iface: &str) {
        let neighbor_id = &iih.source_id;

        // Area validation: for L1 IIHs, area must match (RFC 10589 §8.2.1).
        // For L2 IIHs, area mismatch is allowed but logged.
        if iih.circuit_type & 0x01 != 0 {
            // L1 circuit — check area match
            let neighbor_areas: Vec<String> = iih
                .tlvs
                .iter()
                .filter_map(|t| match t {
                    crate::tlv::IsisTlv::AreaAddresses(addrs) => Some(addrs.clone()),
                    _ => None,
                })
                .flatten()
                .collect();

            let areas_match = neighbor_areas
                .iter()
                .any(|a| self.config.area_addresses.contains(a));

            if !areas_match && !neighbor_areas.is_empty() {
                self.emit(ProtocolEvent::Error {
                    protocol_name: self.name.clone(),
                    message: format!(
                        "IIH from {} on {} area mismatch: neighbor has {:?}, we have {:?}",
                        neighbor_id, iface, neighbor_areas, self.config.area_addresses
                    ),
                });
                // Per RFC 10589, L1 IIH with mismatched area should not form adjacency
                // But we still process it (the adjacency FSM will keep it in Init)
                return;
            }
        }

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

            // On adjacency going Up, trigger LSP generation
            if new_state == crate::adjacency::AdjacencyState::Up {
                // Send our self-LSP immediately so the neighbor can start
                // topology convergence
                let self_lsp = self.lsp_db.generate_self_lsp(
                    &self.config.system_id,
                    &self.adjacencies.values().cloned().collect::<Vec<_>>(),
                    &self.config.area_addresses,
                    &[],   // IP prefixes populated from interface addresses
                    false, // overload bit
                );
                self.send_lsp_packet(&self_lsp).await;

                // Send CSNP on this interface so the neighbor can
                // synchronize its LSDB with ours
                self.send_csnp_on_interface(iface).await;
            }
        }
        self.stats.updates_received += 1;
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
                // Graceful shutdown: purge self-LSP with zero remaining lifetime
                // so neighbors remove us from their LSDBs (RFC 10589 §7.2.10.2)
                let self_lsp = self.lsp_db.generate_self_lsp(
                    &self.config.system_id,
                    &self.adjacencies.values().cloned().collect::<Vec<_>>(),
                    &self.config.area_addresses,
                    &[],   // IP prefixes populated from interface addresses
                    false, // overload bit
                );
                // Override remaining_lifetime to 0 for purge
                let purge_lsp = crate::packet::LspPacket {
                    pdu_length: 0,
                    lsp_id: self_lsp.lsp_id.clone(),
                    sequence_number: self_lsp.sequence_number,
                    remaining_lifetime_secs: 0, // signals purge
                    checksum: self_lsp.checksum,
                    flags: self_lsp.flags.clone(),
                    tlvs: self_lsp.tlvs.clone(),
                };
                self.send_lsp_packet(&purge_lsp).await;

                self.state = ProtocolState::Down;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Down,
                    message: "protocol shutting down (LSP purged)".into(),
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
                self.process_iih(iih, iface).await;
            }
            IsisPacketBody::P2pIih(p2p) => {
                // Convert P2P IIH to a LAN IIH-like structure for adjacency processing.
                // P2P IIH has local_circuit_id instead of priority/lan_id, but the
                // adjacency state machine works the same way.
                let iih = crate::packet::IihPacket {
                    circuit_type: p2p.circuit_type,
                    source_id: p2p.source_id.clone(),
                    holding_time_secs: p2p.holding_time_secs,
                    pdu_length: p2p.pdu_length,
                    priority: 0,       // not used on P2P
                    lan_id: None,      // not used on P2P
                    neighbors: vec![], // P2P 3-way uses TLV, not neighbor list
                    tlvs: p2p.tlvs.clone(),
                };
                self.process_iih(&iih, iface).await;
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
                        overload: lsp.flags.overload,
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

        // Run DIS election on each interface after sending Hellos
        self.run_dis_election();
    }

    /// Run DIS election on all interfaces.
    fn run_dis_election(&mut self) {
        let local_priority = 64u8; // default DIS priority
        let local_system_id = self.config.system_id.clone();

        for iface in &self.config.interfaces {
            let iface_name = iface.interface.clone();
            let iface_adjs: Vec<&Adjacency> = self
                .adjacencies
                .values()
                .filter(|a| a.interface == iface_name)
                .collect();

            let iface_state = self
                .interface_states
                .entry(iface_name.clone())
                .or_insert_with(|| IsisInterfaceState::new(&iface_name));

            let dis_changed = iface_state.elect_dis(&local_system_id, local_priority, &iface_adjs);

            if dis_changed && iface_state.is_dis {
                // We became DIS — generate pseudonode LSP and send CSNP
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Up,
                    message: format!("became DIS on {}", iface_name),
                });
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
                } else {
                    // Track for retransmission on P2P links
                    self.lsp_retrans.add(&iface.interface, &lsp.lsp_id);
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
    async fn process_psnp(&mut self, psnp: &crate::packet::PsnpPacket, received_iface: &str) {
        for entry in &psnp.lsp_entries {
            // PSNP serves as acknowledgment on P2P — remove from retrans list
            self.lsp_retrans.acknowledge(received_iface, &entry.lsp_id);

            // Also send the requested LSP if we have it
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

    /// Generate and send CSNP on all interfaces with Up adjacencies.
    /// The CSNP contains a summary of all LSPs in our LSDB so neighbors
    /// can detect missing or outdated entries.
    async fn csnp_tick(&mut self) {
        if self.transport.is_none() || self.lsp_db.is_empty() {
            return;
        }

        // Build CSNP LSP entries from our LSDB
        let lsp_entries: Vec<crate::packet::CsnpLspEntry> = self
            .lsp_db
            .all()
            .map(|entry| crate::packet::CsnpLspEntry {
                lsp_id: entry.lsp_id.clone(),
                sequence_number: entry.sequence_number,
                remaining_lifetime_secs: entry.remaining_lifetime_secs,
                checksum: entry.checksum,
            })
            .collect();

        // Start and end LSP IDs represent the range covered by this CSNP
        let start_lsp_id = lsp_entries
            .first()
            .map(|e| e.lsp_id.clone())
            .unwrap_or_else(|| crate::packet::LspId::new("0000.0000.0000", 0, 0));
        let end_lsp_id = lsp_entries
            .last()
            .map(|e| e.lsp_id.clone())
            .unwrap_or_else(|| crate::packet::LspId::new("ffff.ffff.ffff", 0xFF, 0xFF));

        let csnp = crate::packet::CsnpPacket {
            pdu_length: 0,
            source_id: self.config.system_id.clone(),
            start_lsp_id: Some(start_lsp_id),
            end_lsp_id: Some(end_lsp_id),
            lsp_entries,
            tlvs: vec![],
        };

        let pkt = IsisPacket {
            header: crate::packet::IsisHeader {
                protocol_id: 0x83,
                header_length: 8,
                version: 1,
                system_id_length: 0,
                pdu_type: PduType::Level2Csnp,
                version2: 1,
                reserved: 0,
                max_area_addresses: 3,
            },
            body: IsisPacketBody::Csnp(csnp),
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

    /// Send a CSNP on a specific interface (used on initial adjacency Up).
    async fn send_csnp_on_interface(&mut self, iface: &str) {
        if self.transport.is_none() || self.lsp_db.is_empty() {
            return;
        }

        let lsp_entries: Vec<crate::packet::CsnpLspEntry> = self
            .lsp_db
            .all()
            .map(|entry| crate::packet::CsnpLspEntry {
                lsp_id: entry.lsp_id.clone(),
                sequence_number: entry.sequence_number,
                remaining_lifetime_secs: entry.remaining_lifetime_secs,
                checksum: entry.checksum,
            })
            .collect();

        let start_lsp_id = lsp_entries
            .first()
            .map(|e| e.lsp_id.clone())
            .unwrap_or_else(|| crate::packet::LspId::new("0000.0000.0000", 0, 0));
        let end_lsp_id = lsp_entries
            .last()
            .map(|e| e.lsp_id.clone())
            .unwrap_or_else(|| crate::packet::LspId::new("ffff.ffff.ffff", 0xFF, 0xFF));

        let csnp = crate::packet::CsnpPacket {
            pdu_length: 0,
            source_id: self.config.system_id.clone(),
            start_lsp_id: Some(start_lsp_id),
            end_lsp_id: Some(end_lsp_id),
            lsp_entries,
            tlvs: vec![],
        };

        let pkt = IsisPacket {
            header: crate::packet::IsisHeader {
                protocol_id: 0x83,
                header_length: 8,
                version: 1,
                system_id_length: 0,
                pdu_type: PduType::Level2Csnp,
                version2: 1,
                reserved: 0,
                max_area_addresses: 3,
            },
            body: IsisPacketBody::Csnp(csnp),
        };

        if let Some(ref transport) = self.transport {
            let _ = transport.send(iface, &pkt).await;
        }
    }

    /// Retransmit LSPs that have not been acknowledged on P2P interfaces.
    async fn lsp_retrans_tick(&mut self) {
        if self.transport.is_none() {
            return;
        }

        for iface in &self.config.interfaces {
            if !self.lsp_retrans.has_pending(&iface.interface) {
                continue;
            }

            let pending_ids = self.lsp_retrans.pending_on(&iface.interface);
            for lsp_key in &pending_ids {
                // Look up the LSP in our database by display key
                if let Some(entry) = self.lsp_db.all().find(|e| e.lsp_id.display() == *lsp_key) {
                    let lsp_pkt = crate::packet::LspPacket {
                        pdu_length: 0,
                        lsp_id: entry.lsp_id.clone(),
                        sequence_number: entry.sequence_number,
                        remaining_lifetime_secs: entry.remaining_lifetime_secs,
                        checksum: entry.checksum,
                        flags: crate::packet::LspFlags::default(),
                        tlvs: entry.tlvs.clone(),
                    };

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
                        body: IsisPacketBody::Lsp(lsp_pkt),
                    };

                    if let Some(ref transport) = self.transport {
                        tracing::debug!(
                            interface = %iface.interface,
                            lsp_id = %lsp_key,
                            "IS-IS LSP retransmission"
                        );
                        let _ = transport.send(&iface.interface, &pkt).await;
                    }
                }
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

            // Initialize interface states
            for iface in &self.config.interfaces {
                self.interface_states
                    .entry(iface.interface.clone())
                    .or_insert_with(|| IsisInterfaceState::new(&iface.interface));
            }
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
                        &self.config.area_addresses,
                        &[],   // IP prefixes populated from interface addresses
                        false, // overload bit
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
                        overload: self_lsp.flags.overload,
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
                _ = self.timers.csnp_interval.tick() => {
                    // DIS routers send CSNPs periodically on broadcast interfaces.
                    // On P2P, CSNPs are only sent on initial adjacency.
                    self.csnp_tick().await;
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
                _ = self.timers.lsp_retrans_interval.tick() => {
                    self.lsp_retrans_tick().await;
                }
            }
        }
    }
}
