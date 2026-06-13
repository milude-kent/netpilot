use netpilot_config::MplsLabelRange;
use std::collections::{BTreeSet, HashMap};
use time::OffsetDateTime;

/// Errors that can occur during label allocation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    #[error("label {0} is outside the domain's configured ranges")]
    OutOfRange(u32),
    #[error("label {0} is already allocated")]
    AlreadyAllocated(u32),
}

/// Source of a label assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelSource {
    /// Explicitly configured by the operator (static label binding).
    Static,
    /// Assigned to a specific protocol instance.
    Protocol { instance_name: String },
    /// Reserved for future use (SR-MPLS dynamic SID allocation, LDP).
    /// Not produced by any code path in this phase.
    #[allow(dead_code)]
    Auto,
}

/// A binding between a FEC (prefix) and an allocated label.
#[derive(Clone, Debug)]
pub struct FecLabelBinding {
    pub prefix: String,
    pub label: u32,
    pub domain: String,
    pub source: LabelSource,
    pub created_at: OffsetDateTime,
}

/// Per-domain label allocation pool.
///
/// Tracks label ranges and allocated labels, providing atomic allocate/free
/// operations. Allocations are not persisted across restarts (matching BIRD2
/// behavior).
#[derive(Clone, Debug)]
pub struct LabelPool {
    ranges: Vec<MplsLabelRange>,
    allocated: BTreeSet<u32>,
}

impl LabelPool {
    /// Create a new pool from the configured label ranges.
    /// Ranges are sorted by `low` on construction.
    pub fn new(ranges: Vec<MplsLabelRange>) -> Self {
        let mut ranges = ranges;
        ranges.sort_by_key(|r| r.low);
        Self {
            ranges,
            allocated: BTreeSet::new(),
        }
    }

    /// Allocate the next available label.
    ///
    /// If `preferred` is `Some(label)` and that label is available AND within
    /// the configured ranges, it is allocated. Otherwise the first free label
    /// is returned. Returns `None` when all labels are exhausted.
    pub fn allocate(&mut self, preferred: Option<u32>) -> Option<u32> {
        if let Some(label) = preferred
            && self.is_in_range(label)
            && !self.allocated.contains(&label)
        {
            self.allocated.insert(label);
            return Some(label);
        }

        for range in &self.ranges {
            for label in range.low..=range.high {
                if !self.allocated.contains(&label) {
                    self.allocated.insert(label);
                    return Some(label);
                }
            }
        }
        None
    }

    /// Reserve a specific static label.
    ///
    /// Returns `Ok(())` on success, or `LabelError` if the label is
    /// out of range or already allocated.
    pub fn allocate_static(&mut self, label: u32) -> Result<(), LabelError> {
        if !self.is_in_range(label) {
            return Err(LabelError::OutOfRange(label));
        }
        if self.allocated.contains(&label) {
            return Err(LabelError::AlreadyAllocated(label));
        }
        self.allocated.insert(label);
        Ok(())
    }

    /// Release a previously allocated label.
    /// No-op if the label was not allocated.
    pub fn free(&mut self, label: u32) {
        self.allocated.remove(&label);
    }

    /// Check whether a label is available (free and in range).
    pub fn is_available(&self, label: u32) -> bool {
        self.is_in_range(label) && !self.allocated.contains(&label)
    }

    /// Return the total number of labels across all ranges.
    pub fn capacity(&self) -> u64 {
        self.ranges
            .iter()
            .map(|r| (r.high - r.low + 1) as u64)
            .sum()
    }

    /// Return the count of currently allocated labels.
    pub fn allocated_count(&self) -> usize {
        self.allocated.len()
    }

    fn is_in_range(&self, label: u32) -> bool {
        self.ranges
            .iter()
            .any(|r| label >= r.low && label <= r.high)
    }
}

/// Collection of label pools keyed by domain name.
#[derive(Clone, Debug, Default)]
pub struct MplsLabelState {
    pub pools: HashMap<String, LabelPool>,
    pub bindings: Vec<FecLabelBinding>,
}

impl MplsLabelState {
    /// Initialize pools from configured MPLS domains.
    pub fn from_domains(domains: &[netpilot_config::MplsDomain]) -> Self {
        let mut state = Self::default();
        for domain in domains {
            state.pools.insert(
                domain.name.clone(),
                LabelPool::new(domain.label_ranges.clone()),
            );
        }
        state
    }

    /// Bind a FEC to a label, recording the binding for CLI queries.
    pub fn bind(&mut self, domain: &str, prefix: &str, label: u32, source: LabelSource) {
        self.bindings.push(FecLabelBinding {
            prefix: prefix.to_string(),
            label,
            domain: domain.to_string(),
            source,
            created_at: OffsetDateTime::now_utc(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_allocates_sequentially() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 20 }]);
        assert_eq!(pool.allocate(None), Some(16));
        assert_eq!(pool.allocate(None), Some(17));
        assert_eq!(pool.allocate(None), Some(18));
        assert_eq!(pool.allocated_count(), 3);
    }

    #[test]
    fn pool_allocate_preferred_label() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 30 }]);
        assert_eq!(pool.allocate(Some(25)), Some(25));
        assert_eq!(pool.allocated_count(), 1);
    }

    #[test]
    fn pool_allocate_ignores_occupied_preferred_label() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 20 }]);
        pool.allocate(Some(17)); // takes 17
        // 17 is taken, should get the next free label
        let got = pool.allocate(Some(17));
        assert!(got.is_some());
        assert_ne!(got, Some(17));
    }

    #[test]
    fn pool_allocate_static_succeeds_for_free_label() {
        let mut pool = LabelPool::new(vec![MplsLabelRange {
            low: 100,
            high: 199,
        }]);
        assert!(pool.allocate_static(150).is_ok());
        assert!(!pool.is_available(150));
    }

    #[test]
    fn pool_allocate_static_fails_for_already_allocated() {
        let mut pool = LabelPool::new(vec![MplsLabelRange {
            low: 100,
            high: 199,
        }]);
        pool.allocate_static(150).unwrap();
        let err = pool
            .allocate_static(150)
            .expect_err("duplicate allocation should fail");
        assert!(matches!(err, LabelError::AlreadyAllocated(150)));
    }

    #[test]
    fn pool_allocate_static_fails_for_out_of_range() {
        let mut pool = LabelPool::new(vec![MplsLabelRange {
            low: 100,
            high: 199,
        }]);
        let err = pool
            .allocate_static(999)
            .expect_err("out-of-range should fail");
        assert!(matches!(err, LabelError::OutOfRange(999)));
    }

    #[test]
    fn pool_free_makes_label_available() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 19 }]);
        pool.allocate_static(17).unwrap();
        assert!(!pool.is_available(17));
        pool.free(17);
        assert!(pool.is_available(17));
    }

    #[test]
    fn pool_returns_none_when_exhausted() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 17 }]);
        assert!(pool.allocate(None).is_some());
        assert!(pool.allocate(None).is_some());
        assert_eq!(pool.allocate(None), None);
    }

    #[test]
    fn pool_capacity_is_sum_of_range_sizes() {
        let pool = LabelPool::new(vec![
            MplsLabelRange { low: 16, high: 25 }, // 10 labels
            MplsLabelRange {
                low: 100,
                high: 109,
            }, // 10 labels
        ]);
        assert_eq!(pool.capacity(), 20);
    }

    #[test]
    fn pool_frees_label_correctly_from_middle_of_range() {
        let mut pool = LabelPool::new(vec![MplsLabelRange {
            low: 100,
            high: 105,
        }]);
        pool.allocate_static(100).unwrap();
        pool.allocate_static(101).unwrap();
        pool.allocate_static(102).unwrap();
        pool.free(101);
        // 101 is now free, 100 and 102 remain allocated
        assert!(pool.is_available(101));
        assert!(!pool.is_available(100));
        assert!(!pool.is_available(102));
    }

    #[test]
    fn mpls_label_state_initializes_pools_from_domains() {
        use netpilot_config::{MplsDomain, MplsLabelRange};

        let domains = vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 100,
                high: 199,
            }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }];

        let state = MplsLabelState::from_domains(&domains);
        assert!(state.pools.contains_key("main"));
        assert_eq!(state.pools.get("main").unwrap().capacity(), 100);
    }
}
