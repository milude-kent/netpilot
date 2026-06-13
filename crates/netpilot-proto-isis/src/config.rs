use serde::{Deserialize, Serialize};

/// Conversion from netpilot_config types into crate-local types
/// (the two crates duplicate these definitions).
impl From<netpilot_config::IsisLevel> for IsisLevel {
    fn from(l: netpilot_config::IsisLevel) -> Self {
        match l {
            netpilot_config::IsisLevel::Level1 => IsisLevel::Level1,
            netpilot_config::IsisLevel::Level2 => IsisLevel::Level2,
            netpilot_config::IsisLevel::Level12 => IsisLevel::Level12,
        }
    }
}

impl From<netpilot_config::IsisInterfaceConfig> for IsisInterfaceConfig {
    fn from(iface: netpilot_config::IsisInterfaceConfig) -> Self {
        IsisInterfaceConfig {
            interface: iface.interface,
            levels: iface.levels.into_iter().map(Into::into).collect(),
            hello_interval_secs: iface.hello_interval_secs,
            hello_multiplier: iface.hello_multiplier,
            metric: iface.metric,
            passive: iface.passive,
            circuit_type: iface.circuit_type.map(Into::into),
            priority: iface.priority,
            sr_adjacency_sid: iface.sr_adjacency_sid,
        }
    }
}

impl From<netpilot_config::CircuitType> for CircuitType {
    fn from(ct: netpilot_config::CircuitType) -> Self {
        match ct {
            netpilot_config::CircuitType::Level1 => CircuitType::Level1,
            netpilot_config::CircuitType::Level2 => CircuitType::Level2,
            netpilot_config::CircuitType::Level12 => CircuitType::Level12,
        }
    }
}

/// IS-IS protocol configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct IsisConfig {
    pub name: String,
    pub table: String,
    pub area_addresses: Vec<String>,
    pub system_id: String,
    pub levels: Vec<IsisLevel>,
    pub interfaces: Vec<IsisInterfaceConfig>,
    pub sr_enabled: Option<bool>,
    // Common protocol fields
    pub limits: Option<netpilot_config::ChannelLimits>,
    pub import_keep_filtered: Option<bool>,
    pub rpki_reload: Option<bool>,
    pub passwords: Option<Vec<netpilot_config::AuthPassword>>,
    pub password: Option<String>,
    pub tx_class: Option<u8>,
    pub tx_priority: Option<u8>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IsisLevel {
    Level1,
    Level2,
    Level12,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct IsisInterfaceConfig {
    pub interface: String,
    pub levels: Vec<IsisLevel>,
    pub hello_interval_secs: Option<u32>,
    pub hello_multiplier: Option<u8>,
    pub metric: Option<u32>,
    pub passive: Option<bool>,
    pub circuit_type: Option<CircuitType>,
    pub priority: Option<u8>,
    pub sr_adjacency_sid: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CircuitType {
    Level1,
    Level2,
    Level12,
}
