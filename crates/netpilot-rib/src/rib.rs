use std::collections::HashMap;
use crate::route::{RouteEntry, RouteKey};
use crate::table::RouteTable;
use crate::nexthop::NextHopResolver;
use netpilot_protocol::event::ProtocolEvent;

/// Core RIB — all routing tables, receiving protocol events.
#[derive(Clone, Debug, Default)]
pub struct RibCore {
    pub tables: HashMap<String, RouteTable>,
    pub resolver: NextHopResolver,
}

impl RibCore {
    pub fn new() -> Self {
        let mut core = Self::default();
        core.tables.insert("master".into(), RouteTable::new("master"));
        core
    }

    /// Get or create a table.
    pub fn table(&mut self, name: &str) -> &mut RouteTable {
        self.tables.entry(name.to_string()).or_insert_with(|| RouteTable::new(name))
    }

    /// Process a protocol event (subscribe to ProtocolSupervisor broadcast).
    pub fn process_event(&mut self, event: &ProtocolEvent) {
        match event {
            ProtocolEvent::RouteAnnounce { table, prefix, next_hop, preference, attributes } => {
                let key = RouteKey::prefix(prefix);
                let entry = RouteEntry::new(key, table, "protocol", *preference)
                    .with_next_hop(crate::route::NextHop::new(next_hop))
                    .with_metric(attributes.metric.unwrap_or(0));
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
        self.tables.get(table).map(|t| t.all_selected().collect()).unwrap_or_default()
    }
}
