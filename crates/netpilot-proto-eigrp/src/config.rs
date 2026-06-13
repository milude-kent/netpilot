use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EigrpConfig {
    pub name: String,
    pub table: String,
    pub autonomous_system: u32,
    pub router_id: String,
    pub interfaces: Vec<EigrpInterfaceConfig>,
    pub k_values: Option<KValues>,
    pub maximum_paths: Option<u32>,
    pub variance: Option<u32>,
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
pub struct KValues {
    pub k1: Option<u8>, // bandwidth (default 1)
    pub k2: Option<u8>, // load (default 0)
    pub k3: Option<u8>, // delay (default 1)
    pub k4: Option<u8>, // reliability (default 0)
    pub k5: Option<u8>, // MTU (default 0)
}

impl Default for KValues {
    fn default() -> Self {
        Self {
            k1: Some(1),
            k2: Some(0),
            k3: Some(1),
            k4: Some(0),
            k5: Some(0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EigrpInterfaceConfig {
    pub interface: String,
    pub hello_interval_secs: Option<u32>,
    pub hold_time_secs: Option<u32>,
    pub bandwidth_kbps: Option<u32>,
    pub delay_tens_of_microseconds: Option<u32>,
    pub passive: Option<bool>,
    pub split_horizon: Option<bool>,
}
