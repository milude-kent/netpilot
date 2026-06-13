use serde::{Deserialize, Serialize};

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
