use routeplane_filter::nettype::Nettype;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RoutePlaneConfig {
    pub schema_version: u32,
    pub identity: RouterIdentity,
    pub tables: Vec<TableConfig>,
    pub protocols: Vec<ProtocolConfig>,
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
            }],
            protocols: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouterIdentity {
    pub router_id: String,
    pub local_asn: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TableConfig {
    pub name: String,
    pub nettype: Option<NettypeDef>,
    pub kernel_table: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProtocolConfig {
    Static {
        name: String,
        table: String,
        routes: Vec<StaticRoute>,
    },
    Bgp {
        name: String,
        table: String,
        local_asn: u32,
        neighbors: Vec<BgpNeighbor>,
    },
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
