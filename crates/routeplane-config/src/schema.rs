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
pub struct StaticRoute {
    pub prefix: String,
    pub next_hop: Option<String>,
    pub blackhole: bool,
    pub address_family: AddressFamily,
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
