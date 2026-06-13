use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Unique key for a route in the RIB.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteKey {
    Prefix { prefix: String, prefix_len: u8 },
    MplsLabel { label: u32 },
}

impl RouteKey {
    pub fn prefix(s: &str) -> Self {
        let parts: Vec<&str> = s.split('/').collect();
        let len = parts.get(1).and_then(|l| l.parse().ok()).unwrap_or(32);
        Self::Prefix {
            prefix: parts[0].to_string(),
            prefix_len: len,
        }
    }

    pub fn label(l: u32) -> Self {
        Self::MplsLabel { label: l }
    }
}

/// A next-hop for a route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextHop {
    pub gateway: String,
    pub interface: Option<String>,
    pub weight: u8,
    pub mpls_labels: Vec<u32>,
}

impl NextHop {
    pub fn new(gateway: &str) -> Self {
        Self {
            gateway: gateway.to_string(),
            interface: None,
            weight: 1,
            mpls_labels: vec![],
        }
    }

    pub fn with_interface(mut self, iface: &str) -> Self {
        self.interface = Some(iface.to_string());
        self
    }

    pub fn with_weight(mut self, w: u8) -> Self {
        self.weight = w;
        self
    }
}

/// A route entry stored in the RIB.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteEntry {
    pub key: RouteKey,
    pub table: String,
    pub source_protocol: String,
    pub preference: u32,
    pub next_hops: Vec<NextHop>,
    pub metric: Option<u32>,
    pub local_pref: Option<u32>,
    pub as_path: Option<Vec<u32>>,
    pub communities: Vec<String>,
    pub mpls_label: Option<u32>,
    pub state: RouteState,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteState {
    Active,
    Hidden,
    Filtered,
    Stale,
    Withdrawn,
}

impl RouteEntry {
    pub fn new(key: RouteKey, table: &str, protocol: &str, preference: u32) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            key,
            table: table.to_string(),
            source_protocol: protocol.to_string(),
            preference,
            next_hops: vec![],
            metric: None,
            local_pref: None,
            as_path: None,
            communities: vec![],
            mpls_label: None,
            state: RouteState::Active,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_next_hop(mut self, nh: NextHop) -> Self {
        self.next_hops.push(nh);
        self
    }
    pub fn with_metric(mut self, m: u32) -> Self {
        self.metric = Some(m);
        self
    }
    pub fn with_as_path(mut self, path: Vec<u32>) -> Self {
        self.as_path = Some(path);
        self
    }
}
