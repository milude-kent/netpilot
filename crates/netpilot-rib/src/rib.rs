use crate::nexthop::NextHopResolver;
use crate::route::{RouteEntry, RouteKey};
use crate::table::RouteTable;
use netpilot_protocol::event::ProtocolEvent;
use std::collections::HashMap;

/// Core RIB — all routing tables, receiving protocol events.
#[derive(Clone, Debug, Default)]
pub struct RibCore {
    pub tables: HashMap<String, RouteTable>,
    pub resolver: NextHopResolver,
}

impl RibCore {
    pub fn new() -> Self {
        let mut core = Self::default();
        core.tables
            .insert("master".into(), RouteTable::new("master"));
        core
    }

    /// Get or create a table.
    pub fn table(&mut self, name: &str) -> &mut RouteTable {
        self.tables
            .entry(name.to_string())
            .or_insert_with(|| RouteTable::new(name))
    }

    /// Process a protocol event (subscribe to ProtocolSupervisor broadcast).
    pub fn process_event(&mut self, event: &ProtocolEvent) {
        match event {
            ProtocolEvent::RouteAnnounce {
                table,
                prefix,
                next_hop,
                preference,
                source_protocol,
                attributes,
            } => {
                let key = RouteKey::prefix(prefix);
                let mut entry = RouteEntry::new(key, table, source_protocol, *preference)
                    .with_next_hop(crate::route::NextHop::new(next_hop));
                if let Some(m) = attributes.metric {
                    entry = entry.with_metric(m);
                }
                if let Some(lp) = attributes.local_pref {
                    entry = entry.with_local_pref(lp);
                }
                if let Some(ref path) = attributes.as_path {
                    entry = entry.with_as_path(path.clone());
                }
                if let Some(ref comms) = attributes.communities {
                    entry = entry.with_communities(comms.clone());
                }
                if let Some(label) = attributes.mpls_label {
                    entry = entry.with_mpls_label(label);
                }
                let tbl = self.table(table);
                tbl.insert(entry);
            }
            ProtocolEvent::RouteWithdraw { table, prefix } => {
                let key = RouteKey::prefix(prefix);
                if let Some(tbl) = self.tables.get_mut(table) {
                    tbl.remove(&key, "protocol");
                }
            }
            _ => {}
        }
    }

    /// Look up a route in a specific table.
    pub fn lookup(&self, table: &str, key: &RouteKey) -> Option<&RouteEntry> {
        self.tables.get(table).and_then(|t| t.lookup(key))
    }

    /// Get all selected routes from a table.
    pub fn all_routes(&self, table: &str) -> Vec<&RouteEntry> {
        self.tables
            .get(table)
            .map(|t| t.all_selected().collect())
            .unwrap_or_default()
    }

    /// Dump all selected routes as JSON string.
    pub fn dump_json(&self) -> Result<String, serde_json::Error> {
        let all: Vec<_> = self
            .tables
            .iter()
            .map(|(name, table)| {
                let routes: Vec<_> = table
                    .all_selected()
                    .map(|e| {
                        serde_json::json!({
                            "table": e.table,
                            "prefix": match &e.key {
                                RouteKey::Prefix { prefix, prefix_len } => {
                                    format!("{}/{}", prefix, prefix_len)
                                }
                                RouteKey::MplsLabel { label } => {
                                    format!("label:{}", label)
                                }
                            },
                            "source": e.source_protocol,
                            "preference": e.preference,
                            "metric": e.metric,
                            "next_hops": e.next_hops.iter().map(|nh| nh.gateway.clone()).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                (name.clone(), routes)
            })
            .collect();
        serde_json::to_string_pretty(&all)
    }

    /// Load routes from a JSON snapshot (placeholder — full implementation requires
    /// reconstructing RouteEntry from saved fields).
    pub fn load_json(&mut self, _json: &str) -> Result<(), serde_json::Error> {
        Ok(()) // Future: deserialize and insert into tables
    }
}
