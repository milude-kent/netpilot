use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use std::collections::HashMap;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{Duration, MissedTickBehavior, interval};

use crate::lsdb::{LsaEntry, LsaType, Lsdb};
use crate::neighbor::{OspfNeighbor, OspfNeighborState};

/// OSPF interface state (per RFC 2328 §9.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OspfInterfaceState {
    Down,
    Loopback,
    Waiting,
    PointToPoint,
    DrOther,
    Backup,
    Dr,
}

/// Per-interface OSPF parameters.
#[derive(Clone, Debug)]
pub struct OspfInterface {
    pub name: String,
    pub area_id: u32,
    pub hello_interval_secs: u16,
    pub dead_interval_secs: u32,
    pub router_priority: u8,
    pub state: OspfInterfaceState,
    pub designated_router: u32,
    pub backup_designated_router: u32,
    pub cost: u32,
}

impl OspfInterface {
    pub fn new(name: &str, area_id: u32) -> Self {
        Self {
            name: name.to_string(),
            area_id,
            hello_interval_secs: 10,
            dead_interval_secs: 40,
            router_priority: 1,
            state: OspfInterfaceState::Down,
            designated_router: 0,
            backup_designated_router: 0,
            cost: 1,
        }
    }
}

#[allow(dead_code)]
pub struct OspfActor {
    name: String,
    table: String,
    router_id: String,
    router_id_u32: u32,
    areas: Vec<String>,
    interfaces: Vec<OspfInterface>,
    lsdb: Lsdb,
    neighbors: HashMap<u32, OspfNeighbor>, // keyed by router_id_u32
    state: ProtocolState,
    stats: ProtocolStats,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
}

impl Default for OspfActor {
    fn default() -> Self {
        Self::new()
    }
}

impl OspfActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            table: String::new(),
            router_id: String::new(),
            router_id_u32: 0,
            areas: vec![],
            interfaces: vec![],
            lsdb: Lsdb::default(),
            neighbors: HashMap::new(),
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            event_tx: None,
        }
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Process a received OSPF Hello packet and update neighbor state.
    #[allow(dead_code)]
    fn handle_hello(&mut self, hello: &netpilot_io::ospf::HelloPacket, iface: &str) {
        let peer_id_u32 = hello.header.router_id;
        let peer_id = netpilot_io::ospf::format_ospf_id(peer_id_u32);
        let router_id_u32 = self.router_id_u32;

        // Find or create the neighbor entry
        let neighbor = self
            .neighbors
            .entry(peer_id_u32)
            .or_insert_with(|| OspfNeighbor::new(&peer_id, iface));

        // Update neighbor fields from the Hello
        neighbor.priority = hello.router_priority;
        neighbor.dead_timer_secs = hello.dead_interval_secs;

        // Check if our router ID is in the neighbor list → TwoWay
        let saw_us = hello.neighbors.contains(&router_id_u32);

        let new_state = if saw_us {
            OspfNeighborState::TwoWay
        } else {
            OspfNeighborState::Init
        };

        if neighbor.state != new_state {
            let old = neighbor.state.clone();
            neighbor.state = new_state.clone();

            // Collect events to emit after releasing the mutable borrow
            let events: Vec<ProtocolEvent> = {
                let mut ev = vec![ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Up,
                    message: format!(
                        "OSPF neighbor {} on {} state {} → {}",
                        peer_id,
                        iface,
                        state_str(&old),
                        state_str(&new_state)
                    ),
                }];

                if new_state == OspfNeighborState::TwoWay {
                    neighbor.state = OspfNeighborState::ExStart;
                    ev.push(ProtocolEvent::StateChange {
                        protocol_name: self.name.clone(),
                        new_state: ProtocolState::Up,
                        message: format!("OSPF neighbor {} on {} starting ExStart", peer_id, iface),
                    });
                }
                ev
            };

            for event in events {
                self.emit(event);
            }

            // Generate LSA after releasing neighbor borrow
            if new_state == OspfNeighborState::TwoWay {
                self.generate_router_lsa_for_neighbor(peer_id_u32, iface);
            }
        }
    }

    /// Process a received OSPF DB Description packet.
    #[allow(dead_code)]
    fn handle_db_desc(&mut self, dd: &netpilot_io::ospf::DbDescPacket) {
        let peer_id_u32 = dd.header.router_id;
        let Some(neighbor) = self.neighbors.get_mut(&peer_id_u32) else {
            return;
        };
        let old_state = neighbor.state.clone();
        let new_state = neighbor.process_db_desc(dd);

        if new_state != old_state {
            self.emit(ProtocolEvent::StateChange {
                protocol_name: self.name.clone(),
                new_state: ProtocolState::Up,
                message: format!(
                    "OSPF neighbor {} DD exchange: {} → {}",
                    netpilot_io::ospf::format_ospf_id(peer_id_u32),
                    state_str(&old_state),
                    state_str(&new_state)
                ),
            });
        }
    }

    /// Handle a received Link State Update packet.
    #[allow(dead_code)]
    fn handle_ls_update(&mut self, update: &netpilot_io::ospf::LsUpdatePacket) {
        for lsa in &update.lsas {
            let advertising_router = lsa.header.advertising_router;
            let link_state_id = lsa.header.link_state_id;
            let ls_type = lsa.header.ls_type;

            let lsa_type = match ls_type {
                1 => LsaType::Router,
                2 => LsaType::Network,
                3 => LsaType::Summary,
                5 => LsaType::AsExternal,
                _ => LsaType::Router, // default
            };

            let _key = format!(
                "{}-{}",
                netpilot_io::ospf::format_ospf_id(link_state_id),
                netpilot_io::ospf::format_ospf_id(advertising_router)
            );

            let area = if ls_type <= 3 {
                Some(self.areas.first().cloned().unwrap_or_default())
            } else {
                None
            };

            // Extract metric from LSA body (simplified)
            let metric = if lsa.body.len() >= 24 {
                // Router LSA: each link is 12 bytes, metric is at offset 8 in each link
                // Simplified: just take the first metric
                Some(u32::from_be_bytes([0, 0, lsa.body[20], lsa.body[21]]))
            } else {
                None
            };

            let entry = LsaEntry {
                link_state_id: netpilot_io::ospf::format_ospf_id(link_state_id),
                advertising_router: netpilot_io::ospf::format_ospf_id(advertising_router),
                sequence_number: lsa.header.ls_sequence_number,
                age_secs: lsa.header.ls_age,
                lsa_type,
                metric,
                area,
            };

            self.lsdb.insert(entry);
        }
    }

    /// Generate a Router LSA entry for a newly-formed adjacency.
    #[allow(dead_code)]
    fn generate_router_lsa_for_neighbor(&mut self, _peer_id: u32, iface: &str) {
        // Create a stub Router LSA representing the adjacency.
        // In a full implementation, this would include all links from the
        // router's interfaces. Here we generate one LSA per adjacency
        // so the SPF algorithm can build paths.
        let _key = format!("{}-{}", self.router_id, iface);
        let entry = LsaEntry {
            link_state_id: self.router_id.clone(),
            advertising_router: self.router_id.clone(),
            sequence_number: 0x80000001,
            age_secs: 0,
            lsa_type: LsaType::Router,
            metric: Some(1),
            area: self.areas.first().cloned(),
        };
        self.lsdb.insert(entry);

        // Also create a stub for the neighbor's router LSA
        let peer_str = netpilot_io::ospf::format_ospf_id(_peer_id);
        let neighbor_entry = LsaEntry {
            link_state_id: peer_str.clone(),
            advertising_router: peer_str,
            sequence_number: 0x80000001,
            age_secs: 0,
            lsa_type: LsaType::Router,
            metric: Some(1),
            area: self.areas.first().cloned(),
        };
        self.lsdb.insert(neighbor_entry);
    }

    /// Build a Hello packet for the given interface.
    fn build_hello(&self, iface: &OspfInterface) -> netpilot_io::ospf::HelloPacket {
        let header = netpilot_io::ospf::OspfHeader {
            version: 2,
            packet_type: 1,
            packet_length: 0, // filled by encode
            router_id: self.router_id_u32,
            area_id: iface.area_id,
            checksum: 0,
            auth_type: 0,
            auth_data: [0u8; 8],
        };

        // Include all known neighbors in the Hello
        let neighbor_ids: Vec<u32> = self.neighbors.keys().copied().collect();

        netpilot_io::ospf::HelloPacket {
            header,
            network_mask: 0xFFFFFF00, // /24 default
            hello_interval_secs: iface.hello_interval_secs,
            options: 0x02, // E-bit
            router_priority: iface.router_priority,
            dead_interval_secs: iface.dead_interval_secs,
            designated_router: iface.designated_router,
            backup_designated_router: iface.backup_designated_router,
            neighbors: neighbor_ids,
        }
    }

    /// Build a DB Description packet for a given neighbor.
    ///
    /// In ExStart: I=1, M=1, MS=1 (we claim master), empty LSA headers.
    /// In Exchange: I=0, M depends on remaining LSAs, MS=1 if master else 0,
    ///   includes LSA headers from our LSDB.
    fn build_db_desc(
        &self,
        neighbor: &OspfNeighbor,
        iface: &OspfInterface,
    ) -> netpilot_io::ospf::DbDescPacket {
        let header = netpilot_io::ospf::OspfHeader {
            version: 2,
            packet_type: 2,   // DB Description
            packet_length: 0, // filled by encode
            router_id: self.router_id_u32,
            area_id: iface.area_id,
            checksum: 0,
            auth_type: 0,
            auth_data: [0u8; 8],
        };

        let (flags, lsa_headers) = match neighbor.state {
            OspfNeighborState::ExStart => {
                // I=1, M=1, MS=1 — initial DD claiming master, no LSA headers
                let flags = 0x07; // I(0x04) | M(0x02) | MS(0x01)
                (flags, Vec::new())
            }
            OspfNeighborState::Exchange => {
                // I=0, M=0 (simplified: send all we have in one packet),
                // MS=1 if we are master else 0
                let ms_bit: u8 = if neighbor.is_master { 0 } else { 1 };
                let m_bit: u8 = 0; // simplified: single packet carries all LSAs
                let flags = m_bit | ms_bit;

                // Collect LSA headers from our LSDB
                let lsa_headers: Vec<netpilot_io::ospf::LsaHeader> = self
                    .lsdb
                    .iter()
                    .map(|(_, entry)| {
                        let ls_type = match entry.lsa_type {
                            LsaType::Router => 1u8,
                            LsaType::Network => 2u8,
                            LsaType::Summary => 3u8,
                            LsaType::AsExternal => 5u8,
                        };
                        let link_state_id =
                            netpilot_io::ospf::parse_ospf_id(&entry.link_state_id).unwrap_or(0);
                        let advertising_router =
                            netpilot_io::ospf::parse_ospf_id(&entry.advertising_router)
                                .unwrap_or(0);

                        netpilot_io::ospf::LsaHeader {
                            ls_age: entry.age_secs,
                            ls_type,
                            link_state_id,
                            advertising_router,
                            ls_sequence_number: entry.sequence_number,
                            ls_checksum: 0, // not computed in this simplified impl
                            length: 20,     // header-only LSAs for now
                        }
                    })
                    .collect();

                (flags, lsa_headers)
            }
            _ => {
                // Should not be called in other states; return empty DD
                (0, Vec::new())
            }
        };

        netpilot_io::ospf::DbDescPacket {
            header,
            interface_mtu: 1500,
            options: 0x02, // E-bit
            flags,
            dd_sequence_number: neighbor.dd_sequence_number,
            lsa_headers,
        }
    }

    /// Send DD packets to all neighbors in ExStart/Exchange state.
    fn send_dd_packets(&self) {
        for (&peer_id, neighbor) in &self.neighbors {
            if !matches!(
                neighbor.state,
                OspfNeighborState::ExStart | OspfNeighborState::Exchange
            ) {
                continue;
            }

            // Find the interface for this neighbor
            let iface = self
                .interfaces
                .iter()
                .find(|i| i.name == neighbor.interface)
                .unwrap_or_else(|| {
                    self.interfaces.first().unwrap_or_else(|| {
                        panic!(
                            "no interface found for neighbor {} on {}",
                            neighbor.router_id, neighbor.interface
                        )
                    })
                });

            let dd = self.build_db_desc(neighbor, iface);
            let _encoded = netpilot_io::ospf::encode_db_desc(&dd);

            tracing::debug!(
                neighbor = %neighbor.router_id,
                state = %state_str(&neighbor.state),
                flags = dd.flags,
                dd_seq = dd.dd_sequence_number,
                lsa_count = dd.lsa_headers.len(),
                "OSPF DB Description built and sent"
            );

            let _ = peer_id; // suppress unused warning
        }
    }

    /// Check for expired neighbors (dead timer).
    fn check_dead_timers(&mut self) {
        // In a full implementation, each neighbor has a dead timer
        // that resets on Hello receipt. Here we just check the state.
        // Timer-based expiry would need per-neighbor timestamps.
    }
}

#[allow(dead_code)]
fn state_str(s: &OspfNeighborState) -> &'static str {
    match s {
        OspfNeighborState::Down => "Down",
        OspfNeighborState::Init => "Init",
        OspfNeighborState::TwoWay => "2-Way",
        OspfNeighborState::ExStart => "ExStart",
        OspfNeighborState::Exchange => "Exchange",
        OspfNeighborState::Loading => "Loading",
        OspfNeighborState::Full => "Full",
    }
}

#[async_trait]
impl ProtocolActor for OspfActor {
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

        if let ProtocolConfig::Ospf {
            table,
            router_id,
            areas,
            ..
        } = &config
        {
            self.table = table.clone();
            self.router_id = router_id.clone().unwrap_or_default();
            self.router_id_u32 = netpilot_io::ospf::parse_ospf_id(&self.router_id).unwrap_or(0);

            // Create interfaces for each area
            for area in areas {
                let area_id = netpilot_io::ospf::parse_ospf_id(&area.area_id).unwrap_or(0);
                // Default interface name derived from area ID
                let iface = OspfInterface::new(&format!("ospf-{}", area.area_id), area_id);
                self.interfaces.push(iface);
                self.areas.push(area.area_id.clone());
            }
        }

        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: format!(
                "OSPF started, router-id {}, {} areas, {} interfaces",
                self.router_id,
                self.areas.len(),
                self.interfaces.len()
            ),
        });

        let hello_interval = Duration::from_secs(10);
        let mut hello_tick = interval(hello_interval);
        hello_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut spf_tick = interval(Duration::from_secs(10));
        spf_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut dead_tick = interval(Duration::from_secs(1));
        dead_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut dd_tick = interval(Duration::from_secs(5));
        dd_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            select! {
                msg = rx.recv() => {
                    match msg {
                        Some(ProtocolMsg::Shutdown) => {
                            return Err(ProtocolError::Stopped(self.name.clone(), "shutdown".into()));
                        }
                        Some(ProtocolMsg::Enable) => {
                            self.state = ProtocolState::Up;
                            // Set all interfaces to waiting/point-to-point
                            for iface in &mut self.interfaces {
                                iface.state = OspfInterfaceState::PointToPoint;
                            }
                        }
                        Some(ProtocolMsg::Disable) => {
                            self.state = ProtocolState::Down;
                            for iface in &mut self.interfaces {
                                iface.state = OspfInterfaceState::Down;
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
                _ = hello_tick.tick() => {
                    // Build and "send" Hello packets on each interface.
                    // In production, these would go via OspfTransport.
                    for iface in &self.interfaces {
                        let hello = self.build_hello(iface);
                        let _encoded = netpilot_io::ospf::encode_hello(&hello);
                        // Transport send would happen here:
                        // transport.send_hello(&iface.name, &encoded).await
                        tracing::debug!(
                            interface = %iface.name,
                            "OSPF Hello built ({} neighbors)",
                            hello.neighbors.len()
                        );
                    }
                    // Also send DD packets for neighbors in ExStart/Exchange
                    self.send_dd_packets();
                }
                _ = dead_tick.tick() => {
                    self.check_dead_timers();
                }
                _ = spf_tick.tick() => {
                    let routes = crate::spf::compute_ospf_spf(&self.lsdb, &self.router_id);
                    for route in &routes {
                        self.emit(ProtocolEvent::RouteAnnounce {
                            table: self.table.clone(),
                            prefix: route.prefix.clone(),
                            next_hop: route.next_hop.clone(),
                            preference: 110,
                            source_protocol: "ospf".into(),
                            attributes: RouteAttributes {
                                metric: Some(route.cost),
                                ..Default::default()
                            },
                        });
                    }
                }
                _ = dd_tick.tick() => {
                    // Periodically send DD packets to neighbors in ExStart/Exchange
                    self.send_dd_packets();
                }
            }
        }
    }
}
