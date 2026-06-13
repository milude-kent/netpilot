use crate::tlv::EigrpMetric;
use std::collections::HashMap;

/// An entry in the EIGRP topology table.
#[derive(Clone, Debug)]
pub struct TopologyEntry {
    pub prefix: String,
    pub feasible_distance: u32, // best metric from this router to the destination
    pub reported_distance: HashMap<String, u32>, // neighbor → their reported distance
    pub via_neighbor: Option<String>, // current successor
    pub feasible_successors: Vec<String>,
    pub metric: EigrpMetric,
}

impl TopologyEntry {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            feasible_distance: u32::MAX,
            reported_distance: HashMap::new(),
            via_neighbor: None,
            feasible_successors: Vec::new(),
            metric: EigrpMetric::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TopologyTable {
    entries: HashMap<String, TopologyEntry>,
}

impl TopologyTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, prefix: &str) -> Option<&TopologyEntry> {
        self.entries.get(prefix)
    }

    pub fn get_mut(&mut self, prefix: &str) -> &mut TopologyEntry {
        self.entries
            .entry(prefix.to_string())
            .or_insert_with(|| TopologyEntry::new(prefix))
    }

    /// Run the DUAL algorithm on a prefix after receiving an update from a neighbor.
    /// Returns the new successor (if any) and whether the route is loop-free.
    pub fn dual(
        &mut self,
        prefix: &str,
        from_neighbor: &str,
        reported_distance: u32,
        metric: &EigrpMetric,
    ) -> DualResult {
        let entry = self.get_mut(prefix);
        entry
            .reported_distance
            .insert(from_neighbor.to_string(), reported_distance);

        let new_fd = reported_distance.saturating_add(metric.composite());

        // Check feasibility condition: reported distance < current feasible distance
        if reported_distance < entry.feasible_distance {
            entry.feasible_distance = new_fd.min(entry.feasible_distance);
            entry.via_neighbor = Some(from_neighbor.to_string());
            entry.metric = metric.clone();
            entry.feasible_successors.clear();
            entry.feasible_successors.push(from_neighbor.to_string());
            DualResult {
                new_successor: Some(from_neighbor.to_string()),
                state: DualState::Passive,
            }
        } else if reported_distance == entry.feasible_distance {
            entry.feasible_successors.push(from_neighbor.to_string());
            DualResult {
                new_successor: None,
                state: DualState::Passive,
            }
        } else {
            // Going active — would need to query neighbors in full implementation
            DualResult {
                new_successor: None,
                state: DualState::Active {
                    prefix: prefix.to_string(),
                },
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &TopologyEntry> {
        self.entries.values()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualResult {
    pub new_successor: Option<String>,
    pub state: DualState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DualState {
    Passive,
    Active { prefix: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_accepts_first_route() {
        let mut table = TopologyTable::new();
        let metric = EigrpMetric {
            bandwidth: 1000,
            delay: 100,
            ..Default::default()
        };
        let result = table.dual("10.0.0.0/8", "n1", 500, &metric);
        assert_eq!(result.new_successor, Some("n1".into()));
        assert_eq!(result.state, DualState::Passive);
    }

    #[test]
    fn dual_feasibility_condition_rejects_higher_rd() {
        let mut table = TopologyTable::new();
        let m1 = EigrpMetric {
            bandwidth: 10,
            delay: 1,
            ..Default::default()
        };
        let m2 = EigrpMetric {
            bandwidth: 10,
            delay: 1,
            ..Default::default()
        };
        // First route from n1: RD=100, composite=256*(10+1)=2816, FD=100+2816=2916
        table.dual("10.0.0.0/8", "n1", 100, &m1);
        // Second route from n2: RD=5000 >= FD=2916, should go active
        let result = table.dual("10.0.0.0/8", "n2", 5000, &m2);
        assert!(matches!(result.state, DualState::Active { .. }));
    }

    #[test]
    fn dual_accepts_lower_reported_distance_as_feasible_successor() {
        let mut table = TopologyTable::new();
        let m1 = EigrpMetric {
            bandwidth: 1000,
            delay: 100,
            ..Default::default()
        };
        let m2 = EigrpMetric {
            bandwidth: 500,
            delay: 50,
            ..Default::default()
        };
        table.dual("10.0.0.0/8", "n1", 500, &m1);
        let result = table.dual("10.0.0.0/8", "n2", 100, &m2); // RD=100 < FD=500+256*(1100)=...
        assert_eq!(result.state, DualState::Passive);
    }
}
