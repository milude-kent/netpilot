use netpilot_filter::nettype::Nettype;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RoutePlaneConfig {
    pub schema_version: u32,
    pub identity: RouterIdentity,
    pub tables: Vec<TableConfig>,
    pub protocols: Vec<ProtocolConfig>,
    pub hostname: Option<String>,
    pub defines: Option<Vec<ConstantDef>>,
    pub cli_sockets: Option<Vec<CliSocketConfig>>,
    pub watchdog_warning_secs: Option<u32>,
    pub watchdog_timeout_secs: Option<u32>,
    pub debug_latency: Option<bool>,
    pub debug_latency_limit_micros: Option<u64>,
    pub debug_protocols: Option<String>,
    pub debug_channels: Option<String>,
    pub debug_tables: Option<String>,
    pub debug_commands: Option<u8>,
    pub timeformat_route: Option<String>,
    pub timeformat_protocol: Option<String>,
    pub timeformat_base: Option<String>,
    pub timeformat_log: Option<String>,
}

impl Default for RoutePlaneConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            identity: RouterIdentity::default(),
            tables: vec![TableConfig {
                name: "master".to_string(),
                nettype: None,
                kernel_table: Some(254),
                gc_threshold: None,
                gc_period_secs: None,
                sorted: None,
                trie: None,
                min_settle_time_secs: None,
                max_settle_time_secs: None,
            }],
            protocols: Vec::new(),
            hostname: None,
            defines: None,
            cli_sockets: None,
            watchdog_warning_secs: None,
            watchdog_timeout_secs: None,
            debug_latency: None,
            debug_latency_limit_micros: None,
            debug_protocols: None,
            debug_channels: None,
            debug_tables: None,
            debug_commands: None,
            timeformat_route: None,
            timeformat_protocol: None,
            timeformat_base: None,
            timeformat_log: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouterIdentity {
    pub router_id: String,
    pub local_asn: Option<u32>,
    pub router_id_from: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TableConfig {
    pub name: String,
    pub nettype: Option<NettypeDef>,
    pub kernel_table: Option<u32>,
    pub gc_threshold: Option<u32>,
    pub gc_period_secs: Option<u32>,
    pub sorted: Option<bool>,
    pub trie: Option<bool>,
    pub min_settle_time_secs: Option<u32>,
    pub max_settle_time_secs: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProtocolConfig {
    Static {
        name: String,
        table: String,
        routes: Vec<StaticRoute>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
    },
    Bgp {
        name: String,
        table: String,
        local_asn: u32,
        neighbors: Vec<BgpNeighbor>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ChannelLimits {
    pub import_limit: Option<u32>,
    pub import_limit_action: Option<LimitAction>,
    pub receive_limit: Option<u32>,
    pub receive_limit_action: Option<LimitAction>,
    pub export_limit: Option<u32>,
    pub export_limit_action: Option<LimitAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LimitAction {
    Warn,
    Block,
    Restart,
    Disable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuthPassword {
    pub id: Option<u8>,
    pub password: String,
    pub generate_from: Option<String>,
    pub generate_to: Option<String>,
    pub accept_from: Option<String>,
    pub accept_to: Option<String>,
    pub algorithm: Option<AuthAlgorithm>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthAlgorithm {
    KeyedMd5,
    KeyedSha1,
    HmacSha1,
    HmacSha256,
    HmacSha384,
    HmacSha512,
    Blake2s128,
    Blake2s256,
    Blake2b256,
    Blake2b512,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConstantDef {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StaticNexthopType {
    Router,
    Blackhole,
    Unreachable,
    Prohibit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StaticRoute {
    pub prefix: String,
    pub next_hop: Option<String>,
    pub blackhole: bool,
    pub address_family: AddressFamily,
    pub nexthop_type: Option<StaticNexthopType>,
    pub mpls_label: Option<u32>,
    pub igp_metric: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BgpNeighbor {
    pub name: String,
    pub remote_address: String,
    pub remote_asn: u32,
    pub address_families: Vec<AddressFamily>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NettypeDef {
    Ip4,
    Ip6,
    Ip6Sadr,
    Vpn4,
    Vpn6,
    Roa4,
    Roa6,
    Aspa,
    Flow4,
    Flow6,
    Eth,
    Mpls,
    Evpn,
    Neighbor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CliSocketConfig {
    pub path: String,
    pub restrict: Option<bool>,
}

impl From<NettypeDef> for Nettype {
    fn from(def: NettypeDef) -> Self {
        match def {
            NettypeDef::Ip4 => Nettype::Ip4,
            NettypeDef::Ip6 => Nettype::Ip6,
            NettypeDef::Ip6Sadr => Nettype::Ip6Sadr,
            NettypeDef::Vpn4 => Nettype::Vpn4,
            NettypeDef::Vpn6 => Nettype::Vpn6,
            NettypeDef::Roa4 => Nettype::Roa4,
            NettypeDef::Roa6 => Nettype::Roa6,
            NettypeDef::Aspa => Nettype::Aspa,
            NettypeDef::Flow4 => Nettype::Flow4,
            NettypeDef::Flow6 => Nettype::Flow6,
            NettypeDef::Eth => Nettype::Eth,
            NettypeDef::Mpls => Nettype::Mpls,
            NettypeDef::Evpn => Nettype::Evpn,
            NettypeDef::Neighbor => Nettype::Neighbor,
        }
    }
}
