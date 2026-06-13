use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProtocolEvent {
    RouteAnnounce {
        table: String,
        prefix: String,
        next_hop: String,
        preference: u32,
        attributes: RouteAttributes,
    },
    RouteWithdraw {
        table: String,
        prefix: String,
    },
    StateChange {
        protocol_name: String,
        new_state: ProtocolState,
        message: String,
    },
    Error {
        protocol_name: String,
        message: String,
    },
    Stats {
        protocol_name: String,
        stats: ProtocolStats,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolState {
    Down,
    Start,
    Up,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStatus {
    pub name: String,
    pub state: ProtocolState,
    pub uptime_secs: u64,
    pub routes_imported: u64,
    pub routes_exported: u64,
}

impl ProtocolStatus {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: ProtocolState::Down,
            uptime_secs: 0,
            routes_imported: 0,
            routes_exported: 0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub routes_imported: u64,
    pub routes_exported: u64,
    pub routes_filtered: u64,
    pub updates_received: u64,
    pub updates_sent: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteAttributes {
    pub local_pref: Option<u32>,
    pub metric: Option<u32>,
    pub as_path: Option<Vec<u32>>,
    pub communities: Option<Vec<String>>,
    pub mpls_label: Option<u32>,
}
