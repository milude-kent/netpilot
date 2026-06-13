use std::collections::HashMap;
use crate::route::{RouteEntry, RouteKey};
use crate::selection::select_best;

/// Per-table route storage with best-route selection.
#[derive(Clone, Debug, Default)]
pub struct RouteTable {
    pub name: String,
    /// All routes, keyed by RouteKey. Multiple protocols can contribute routes to the same key.
    pub routes: HashMap<RouteKey, Vec<RouteEntry>>,
    /// Currently selected best route for each key.
    pub selected: HashMap<RouteKey, RouteEntry>,
    pub route_count: usize,
}

impl RouteTable {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), routes: HashMap::new(), selected: HashMap::new(), route_count: 0 }
    }

    /// Insert a route. Runs best-route selection after insert.
    pub fn insert(&mut self, entry: RouteEntry) -> &RouteEntry {
        let key = entry.key.clone();
        let entries = self.routes.entry(key.clone()).or_default();
        entries.push(entry);
        self.route_count += 1;
        // Re-select best route for this key
        if let Some(best) = select_best(entries) {
            self.selected.insert(key.clone(), best.clone());
        }
        self.selected.get(&key).unwrap_or_else(|| entries.last().unwrap())
    }

    /// Remove all routes from a protocol for a given key. Returns count removed.
    pub fn remove(&mut self, key: &RouteKey, protocol: &str) -> usize {
        if let Some(entries) = self.routes.get_mut(key) {
            let before = entries.len();
            entries.retain(|e| e.source_protocol != protocol);
            let removed = before - entries.len();
            self.route_count -= removed;
            if entries.is_empty() {
                self.routes.remove(key);
                self.selected.remove(key);
            } else if let Some(best) = select_best(entries) {
                self.selected.insert(key.clone(), best.clone());
            }
            removed
        } else { 0 }
    }

    /// Look up the selected route for a key.
    pub fn lookup(&self, key: &RouteKey) -> Option<&RouteEntry> {
        self.selected.get(key)
    }

    /// Iterate over all selected routes.
    pub fn all_selected(&self) -> impl Iterator<Item = &RouteEntry> {
        self.selected.values()
    }

    pub fn len(&self) -> usize { self.selected.len() }
}
