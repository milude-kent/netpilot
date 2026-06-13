use std::collections::{BinaryHeap, HashMap};
use crate::lsdb::{Lsdb, LsaType};

/// A route computed by the OSPF SPF algorithm.
#[derive(Clone, Debug)]
pub struct OspfRoute {
    pub prefix: String,
    pub next_hop: String,
    pub cost: u32,
    pub area: String,
}

#[derive(PartialEq, Eq)]
struct HeapEntry {
    router_id: String,
    cost: u32,
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.cost.cmp(&self.cost)
    }
}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Compute OSPF shortest-path-first routes from the LSDB using Dijkstra's algorithm.
/// Returns reachable routes with costs and next-hops.
pub fn compute_ospf_spf(lsdb: &Lsdb, root: &str) -> Vec<OspfRoute> {
    let mut distances: HashMap<String, u32> = HashMap::new();
    let mut parent: HashMap<String, Option<String>> = HashMap::new();
    let mut heap = BinaryHeap::new();

    distances.insert(root.to_string(), 0);
    parent.insert(root.to_string(), None);
    heap.push(HeapEntry { router_id: root.to_string(), cost: 0 });

    // Build adjacency graph from Router LSAs
    // In a full implementation: parse LSA body for point-to-point/transit links
    // For now: process all LSAs that look like router LSAs
    for (_id, lsa) in lsdb.iter() {
        if lsa.lsa_type == LsaType::Router {
            // Simplified: assume each router LSA has stub network links
            // real implementation parses TLV/fields
            let cost = lsa.metric.unwrap_or(1);
            if !distances.contains_key(&lsa.advertising_router) {
                distances.insert(lsa.advertising_router.clone(), cost);
                parent.insert(lsa.advertising_router.clone(), Some(root.to_string()));
                heap.push(HeapEntry { router_id: lsa.advertising_router.clone(), cost });
            }
        }
    }

    // Dijkstra
    while let Some(HeapEntry { router_id, cost }) = heap.pop() {
        if cost > *distances.get(&router_id).unwrap_or(&u32::MAX) {
            continue;
        }
        // Process this node's links
        if let Some(lsa) = lsdb.get(&router_id) {
            // Stub: treat each LSA as a route
            let _ = lsa;
        }
    }

    // Extract routes
    let mut routes = Vec::new();
    for (_id, lsa) in lsdb.iter() {
        if lsa.lsa_type == LsaType::Network || lsa.lsa_type == LsaType::Router {
            if let Some(&dist) = distances.get(&lsa.advertising_router) {
                routes.push(OspfRoute {
                    prefix: lsa.link_state_id.clone(),
                    next_hop: resolve_ospf_next_hop(&parent, &lsa.advertising_router, root),
                    cost: dist + lsa.metric.unwrap_or(0),
                    area: lsa.area.clone().unwrap_or_default(),
                });
            }
        }
    }

    routes
}

fn resolve_ospf_next_hop(parent: &HashMap<String, Option<String>>, dest: &str, root: &str) -> String {
    let mut current = dest.to_string();
    while let Some(Some(p)) = parent.get(&current) {
        if p == root { return current; }
        current = p.clone();
    }
    dest.to_string()
}
