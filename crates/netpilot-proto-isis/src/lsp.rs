use crate::adjacency::Adjacency;
use crate::packet::{LspId, LspPacket};
use crate::tlv::{ExtendedNeighbor, IsisTlv};
use std::collections::{HashMap, HashSet};
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct LspEntry {
    pub lsp_id: LspId,
    pub sequence_number: u32,
    pub remaining_lifetime_secs: u16,
    pub checksum: u16,
    pub tlvs: Vec<IsisTlv>,
    pub received_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Clone, Debug, Default)]
pub struct LspDatabase {
    lsps: HashMap<String, LspEntry>, // keyed by LspId::display()
}

impl LspDatabase {
    pub fn new() -> Self {
        Self {
            lsps: HashMap::new(),
        }
    }

    pub fn insert(&mut self, entry: LspEntry) {
        let key = entry.lsp_id.display();
        self.lsps.insert(key, entry);
    }

    pub fn get(&self, id: &LspId) -> Option<&LspEntry> {
        self.lsps.get(&id.display())
    }

    pub fn get_mut(&mut self, id: &LspId) -> Option<&mut LspEntry> {
        self.lsps.get_mut(&id.display())
    }

    pub fn contains_newer(&self, id: &LspId, seq: u32) -> bool {
        self.get(id).is_some_and(|e| e.sequence_number >= seq)
    }

    /// Generate a self-LSP from local adjacency state.
    pub fn generate_self_lsp(&self, system_id: &str, adjacencies: &[Adjacency]) -> LspPacket {
        let up_adjs: Vec<&Adjacency> = adjacencies.iter().filter(|a| a.is_up()).collect();
        let neighbors: Vec<ExtendedNeighbor> = up_adjs
            .iter()
            .map(|a| ExtendedNeighbor {
                system_id: a.neighbor_system_id.clone(),
                metric: 10,
                pseudonode_id: 0,
            })
            .collect();

        let tlvs = vec![
            IsisTlv::Hostname(system_id.to_string()),
            IsisTlv::AreaAddresses(vec!["49.0001".to_string()]),
            IsisTlv::ExtendedIsReachability(neighbors),
        ];

        let existing = self
            .lsps
            .values()
            .find(|e| e.lsp_id.system_id == system_id && e.lsp_id.pseudonode_id == 0);

        LspPacket {
            pdu_length: 0,
            remaining_lifetime_secs: 1200,
            lsp_id: LspId::new(system_id, 0, 0),
            sequence_number: existing.map_or(1, |e| e.sequence_number + 1),
            checksum: 0,
            flags: Default::default(),
            tlvs,
        }
    }

    /// Purge expired LSPs. Returns count of purged entries.
    pub fn purge_expired(&mut self) -> usize {
        let before = self.lsps.len();
        let now = OffsetDateTime::now_utc();
        self.lsps.retain(|_, e| e.expires_at > now);
        before - self.lsps.len()
    }

    pub fn all(&self) -> impl Iterator<Item = &LspEntry> {
        self.lsps.values()
    }

    pub fn len(&self) -> usize {
        self.lsps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lsps.is_empty()
    }
}

/// Tracks LSPs that need retransmission on each interface.
/// On P2P links, LSPs are retransmitted until acknowledged by PSNP.
/// On broadcast links, CSNPs serve as implicit acknowledgment.
#[derive(Clone, Debug, Default)]
pub struct LspRetransmissionList {
    /// interface -> set of LSP IDs awaiting acknowledgment
    pending: HashMap<String, HashSet<String>>,
}

impl LspRetransmissionList {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Add an LSP to the retransmission list for an interface.
    pub fn add(&mut self, iface: &str, lsp_id: &LspId) {
        self.pending
            .entry(iface.to_string())
            .or_default()
            .insert(lsp_id.display());
    }

    /// Remove an LSP from the retransmission list (acknowledged by PSNP).
    pub fn acknowledge(&mut self, iface: &str, lsp_id: &LspId) {
        if let Some(set) = self.pending.get_mut(iface) {
            set.remove(&lsp_id.display());
        }
    }

    /// Get all LSP IDs pending retransmission on an interface.
    pub fn pending_on(&self, iface: &str) -> Vec<String> {
        self.pending
            .get(iface)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if there are any pending retransmissions on an interface.
    pub fn has_pending(&self, iface: &str) -> bool {
        self.pending.get(iface).is_some_and(|s| !s.is_empty())
    }

    /// Remove all pending entries for an interface (e.g., adjacency went down).
    pub fn clear_interface(&mut self, iface: &str) {
        self.pending.remove(iface);
    }
}
