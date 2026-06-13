use crate::lsp::LspDatabase;
use crate::tlv::IsisTlv;
use std::collections::{BinaryHeap, HashMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpfResult {
    pub nodes: Vec<SpfNode>,
    pub routes: Vec<SpfRoute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpfNode {
    pub system_id: String,
    pub pseudonode: bool,
    pub distance: u32,
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpfRoute {
    pub prefix: String,
    pub next_hop: String,
    pub metric: u32,
}

#[derive(PartialEq, Eq)]
struct HeapEntry {
    system_id: String,
    distance: u32,
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.distance.cmp(&self.distance) // reverse for min-heap
    }
}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Compute Dijkstra SPF from the LSP database using `root` as source.
/// Returns reachable nodes and their routes along with next-hops.
pub fn compute_spf(lsp_db: &LspDatabase, root: &str) -> SpfResult {
    let mut distances: HashMap<String, u32> = HashMap::new();
    let mut parent: HashMap<String, Option<String>> = HashMap::new();
    let mut heap = BinaryHeap::new();

    distances.insert(root.to_string(), 0);
    parent.insert(root.to_string(), None);
    heap.push(HeapEntry {
        system_id: root.to_string(),
        distance: 0,
    });

    // Collect all edges from the LSP database
    let mut edges: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    for entry in lsp_db.all() {
        let src = entry.lsp_id.system_id.clone();
        for tlv in &entry.tlvs {
            if let IsisTlv::ExtendedIsReachability(neighbors) = tlv {
                for n in neighbors {
                    let dst = n.system_id.clone();
                    edges
                        .entry(src.clone())
                        .or_default()
                        .push((dst.clone(), n.metric));
                    edges.entry(dst).or_default().push((src.clone(), n.metric));
                }
            }
        }
    }

    // Dijkstra algorithm
    while let Some(HeapEntry {
        system_id,
        distance,
    }) = heap.pop()
    {
        if distance > *distances.get(&system_id).unwrap_or(&u32::MAX) {
            continue;
        }
        if let Some(neighbors) = edges.get(&system_id) {
            for (neighbor, metric) in neighbors {
                let new_dist = distance + metric;
                if new_dist < *distances.get(neighbor).unwrap_or(&u32::MAX) {
                    distances.insert(neighbor.clone(), new_dist);
                    parent.insert(neighbor.clone(), Some(system_id.clone()));
                    heap.push(HeapEntry {
                        system_id: neighbor.clone(),
                        distance: new_dist,
                    });
                }
            }
        }
    }

    // Build nodes list
    let nodes: Vec<SpfNode> = distances
        .iter()
        .map(|(id, d)| SpfNode {
            system_id: id.clone(),
            pseudonode: false,
            distance: *d,
            parent: parent.get(id).and_then(|p| p.clone()),
        })
        .collect();

    // Extract routes from node LSPs (IP reachability TLVs)
    let mut routes = Vec::new();
    for entry in lsp_db.all() {
        let src = &entry.lsp_id.system_id;
        if let Some(&dist) = distances.get(src) {
            for tlv in &entry.tlvs {
                match tlv {
                    IsisTlv::IpInternalReachability(entries) => {
                        for e in entries {
                            routes.push(SpfRoute {
                                prefix: e.prefix.clone(),
                                next_hop: resolve_next_hop(&parent, src, root),
                                metric: dist + e.metric,
                            });
                        }
                    }
                    IsisTlv::PrefixSid(entry_sid) => {
                        routes.push(SpfRoute {
                            prefix: entry_sid.prefix.clone(),
                            next_hop: resolve_next_hop(&parent, src, root),
                            metric: dist,
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    SpfResult { nodes, routes }
}

fn resolve_next_hop(parent: &HashMap<String, Option<String>>, dest: &str, root: &str) -> String {
    let mut current = dest.to_string();
    while let Some(Some(p)) = parent.get(&current) {
        if p == root {
            return current; // This is the direct neighbor / next-hop
        }
        current = p.clone();
    }
    dest.to_string() // fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::{LspDatabase, LspEntry};
    use crate::packet::LspId;
    use crate::tlv::{ExtendedNeighbor, IpReachEntry, IsisTlv};
    use time::OffsetDateTime;

    fn make_entry(system_id: &str, neighbors: Vec<(&str, u32)>, prefixes: Vec<&str>) -> LspEntry {
        let extended: Vec<ExtendedNeighbor> = neighbors
            .iter()
            .map(|(id, m)| ExtendedNeighbor {
                system_id: id.to_string(),
                metric: *m,
                pseudonode_id: 0,
            })
            .collect();
        let ip_reach: Vec<IpReachEntry> = prefixes
            .iter()
            .map(|p| IpReachEntry {
                prefix: p.to_string(),
                metric: 0,
                up_down: false,
                sub_tlv: false,
                prefix_len: 24,
            })
            .collect();

        let mut tlvs = vec![IsisTlv::ExtendedIsReachability(extended)];
        if !ip_reach.is_empty() {
            tlvs.push(IsisTlv::IpInternalReachability(ip_reach));
        }
        let now = OffsetDateTime::now_utc();
        LspEntry {
            lsp_id: LspId::new(system_id, 0, 0),
            sequence_number: 1,
            remaining_lifetime_secs: 1200,
            checksum: 0,
            tlvs,
            received_at: now,
            expires_at: now + time::Duration::seconds(1200),
        }
    }

    #[test]
    fn spf_single_node_has_zero_distance() {
        let mut db = LspDatabase::new();
        db.insert(make_entry("A", vec![], vec!["10.0.0.0/24"]));
        let result = compute_spf(&db, "A");
        let a = result.nodes.iter().find(|n| n.system_id == "A").unwrap();
        assert_eq!(a.distance, 0);
        assert_eq!(result.routes.len(), 1);
    }

    #[test]
    fn spf_triangle_topology() {
        let mut db = LspDatabase::new();
        db.insert(make_entry(
            "A",
            vec![("B", 10), ("C", 20)],
            vec!["10.0.0.0/24"],
        ));
        db.insert(make_entry(
            "B",
            vec![("A", 10), ("C", 10)],
            vec!["10.0.1.0/24"],
        ));
        db.insert(make_entry("C", vec![("A", 20), ("B", 10)], vec![]));
        let result = compute_spf(&db, "A");
        let b = result.nodes.iter().find(|n| n.system_id == "B").unwrap();
        assert_eq!(b.distance, 10);
        let c = result.nodes.iter().find(|n| n.system_id == "C").unwrap();
        assert_eq!(c.distance, 20); // A→B→C = 20, direct A→C = 20, tie
    }
}
