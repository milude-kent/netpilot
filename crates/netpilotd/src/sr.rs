use netpilot_config::{RoutePlaneConfig, SrAdjSidType, SrSidType};

/// In-memory registry of SR SIDs, loaded from config.
#[derive(Clone, Debug, Default)]
pub struct SidRegistry {
    pub prefix_sids: Vec<SrPrefixSidEntry>,
    pub adjacency_sids: Vec<SrAdjacencySidEntry>,
}

#[derive(Clone, Debug)]
pub struct SrPrefixSidEntry {
    pub prefix: String,
    pub label: u32,
    pub domain: String,
}

#[derive(Clone, Debug)]
pub struct SrAdjacencySidEntry {
    pub interface: String,
    pub neighbor: String,
    pub label: u32,
    pub domain: String,
}

impl SidRegistry {
    /// Build the registry from a RoutePlaneConfig, resolving Index SIDs to
    /// absolute labels using each domain's SRGB.
    pub fn from_config(config: &RoutePlaneConfig) -> Self {
        let mut registry = Self::default();

        if let Some(sids) = &config.sr_prefix_sids {
            for sid in sids {
                let label = match &sid.sid_type {
                    SrSidType::Absolute(l) => *l,
                    SrSidType::Index(idx) => resolve_index_in_srgb(config, &sid.domain, *idx),
                };
                registry.prefix_sids.push(SrPrefixSidEntry {
                    prefix: sid.prefix.clone(),
                    label,
                    domain: sid.domain.clone(),
                });
            }
        }

        if let Some(sids) = &config.sr_adjacency_sids {
            for sid in sids {
                let label = match &sid.sid_type {
                    SrAdjSidType::Absolute(l) => *l,
                    SrAdjSidType::Dynamic => 0,
                };
                registry.adjacency_sids.push(SrAdjacencySidEntry {
                    interface: sid.interface.clone(),
                    neighbor: sid.neighbor.clone(),
                    label,
                    domain: sid.domain.clone(),
                });
            }
        }

        registry
    }

    /// Resolve a destination prefix to its prefix-SID label via longest prefix match.
    pub fn resolve_prefix_sid(&self, prefix: &str) -> Option<u32> {
        self.prefix_sids
            .iter()
            .filter(|e| prefix_starts_with(prefix, &e.prefix))
            .max_by_key(|e| prefix_length(&e.prefix))
            .map(|e| e.label)
    }

    pub fn list_prefix_sids(&self) -> &[SrPrefixSidEntry] {
        &self.prefix_sids
    }

    pub fn list_adjacency_sids(&self) -> &[SrAdjacencySidEntry] {
        &self.adjacency_sids
    }
}

/// Compute an MPLS label stack for a given destination.
/// Returns single-label stack for prefix-SID matches; IGP topology integration in M6.
pub fn compute_label_stack(registry: &SidRegistry, destination: &str) -> Option<Vec<u32>> {
    registry.resolve_prefix_sid(destination).map(|label| vec![label])
}

fn resolve_index_in_srgb(config: &RoutePlaneConfig, domain_name: &str, index: u32) -> u32 {
    if let Some(domains) = &config.mpls_domains {
        if let Some(d) = domains.iter().find(|d| d.name == domain_name) {
            if let Some(ref srgb) = d.sr_global_block {
                return srgb.low + index;
            }
        }
    }
    index
}

fn prefix_starts_with(addr: &str, prefix: &str) -> bool {
    let prefix_base = prefix.split('/').next().unwrap_or(prefix);
    let addr_base = addr.split('/').next().unwrap_or(addr);
    addr_base == prefix_base
}

fn prefix_length(prefix: &str) -> usize {
    prefix
        .split('/')
        .nth(1)
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use netpilot_config::{MplsDomain, MplsLabelRange};

    fn make_test_config() -> RoutePlaneConfig {
        RoutePlaneConfig {
            mpls_domains: Some(vec![MplsDomain {
                name: "main".into(),
                label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: Some(true),
                sr_global_block: Some(MplsLabelRange { low: 16000, high: 24000 }),
                static_bindings: None,
            }]),
            sr_prefix_sids: Some(vec![
                netpilot_config::SrPrefixSidConfig {
                    prefix: "10.0.0.0/8".into(),
                    domain: "main".into(),
                    sid_type: SrSidType::Index(0),
                    flags: netpilot_config::SrPrefixSidFlags {
                        n_flag_clear: None,
                        php: None,
                        explicit_null: None,
                    },
                },
                netpilot_config::SrPrefixSidConfig {
                    prefix: "192.168.0.0/16".into(),
                    domain: "main".into(),
                    sid_type: SrSidType::Absolute(17000),
                    flags: netpilot_config::SrPrefixSidFlags {
                        n_flag_clear: None,
                        php: None,
                        explicit_null: None,
                    },
                },
            ]),
            ..RoutePlaneConfig::default()
        }
    }

    #[test]
    fn sid_registry_loads_prefix_sids_from_config() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.list_prefix_sids().len(), 2);
    }

    #[test]
    fn resolve_prefix_sid_finds_exact_match() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), Some(16000));
    }

    #[test]
    fn resolve_prefix_sid_returns_none_for_unknown() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.resolve_prefix_sid("172.16.0.0/12"), None);
    }

    #[test]
    fn compute_label_stack_returns_single_label() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        let stack = compute_label_stack(&registry, "10.0.0.0/8");
        assert_eq!(stack, Some(vec![16000]));
    }

    #[test]
    fn compute_label_stack_returns_none_for_unknown() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        let stack = compute_label_stack(&registry, "172.16.0.0/12");
        assert_eq!(stack, None);
    }

    #[test]
    fn absolute_sid_is_used_directly() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.resolve_prefix_sid("192.168.0.0/16"), Some(17000));
    }

    #[test]
    fn index_sid_resolves_against_srgb() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), Some(16000));
    }

    #[test]
    fn adjacency_sid_dynamic_sets_label_to_zero() {
        let config = RoutePlaneConfig {
            mpls_domains: Some(vec![MplsDomain {
                name: "main".into(),
                label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: Some(true),
                sr_global_block: Some(MplsLabelRange { low: 16000, high: 24000 }),
                static_bindings: None,
            }]),
            sr_adjacency_sids: Some(vec![netpilot_config::SrAdjacencySidConfig {
                interface: "eth0".into(),
                neighbor: "192.0.2.1".into(),
                domain: "main".into(),
                sid_type: SrAdjSidType::Dynamic,
                protected: false,
            }]),
            ..RoutePlaneConfig::default()
        };
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.list_adjacency_sids().len(), 1);
        assert_eq!(registry.list_adjacency_sids()[0].label, 0);
    }
}
