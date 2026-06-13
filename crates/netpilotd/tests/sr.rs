use netpilot_config::{
    MplsDomain, MplsLabelRange, RoutePlaneConfig, RouterIdentity, SrPrefixSidConfig,
    SrPrefixSidFlags, SrSidType,
};
use netpilotd::sr::SidRegistry;

#[test]
fn sid_registry_from_config_with_srgb_resolves_index_correctly() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 16000,
                high: 24000,
            }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: Some(MplsLabelRange {
                low: 16000,
                high: 24000,
            }),
            static_bindings: None,
        }]),
        sr_prefix_sids: Some(vec![
            SrPrefixSidConfig {
                prefix: "10.0.0.0/8".into(),
                domain: "main".into(),
                sid_type: SrSidType::Index(1),
                flags: SrPrefixSidFlags {
                    n_flag_clear: None,
                    php: None,
                    explicit_null: None,
                },
            },
            SrPrefixSidConfig {
                prefix: "172.16.0.0/12".into(),
                domain: "main".into(),
                sid_type: SrSidType::Absolute(17000),
                flags: SrPrefixSidFlags {
                    n_flag_clear: None,
                    php: Some(true),
                    explicit_null: None,
                },
            },
        ]),
        ..RoutePlaneConfig::default()
    };

    let registry = SidRegistry::from_config(&config);

    // Index-based SID resolves as SRGB.low + index = 16000 + 1 = 16001
    assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), Some(16001));
    // Absolute SID used directly
    assert_eq!(registry.resolve_prefix_sid("172.16.0.0/12"), Some(17000));
    // Unknown prefix
    assert_eq!(registry.resolve_prefix_sid("192.168.0.0/16"), None);
}

#[test]
fn empty_config_creates_empty_registry() {
    let config = RoutePlaneConfig::default();
    let registry = SidRegistry::from_config(&config);
    assert!(registry.list_prefix_sids().is_empty());
    assert!(registry.list_adjacency_sids().is_empty());
    assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), None);
}
