use netpilot_config::{
    AddressFamily, CommitRequest, ConfigStore, MplsChannelConfig, MplsDomain, MplsLabelPolicy,
    MplsLabelRange, MplsStaticBinding, MplsTableConfig, ProtocolConfig, RollbackRequest,
    RoutePlaneConfig, RouterIdentity, StaticNexthopType, StaticRoute, TableConfig,
    diff::ConfigDiff, validation::validate_config,
};

#[test]
fn default_config_has_main_table_and_schema_version() {
    let config = RoutePlaneConfig::default();

    assert_eq!(config.schema_version, 1);
    assert_eq!(config.tables.len(), 1);
    assert_eq!(config.tables[0].name, "master");
}

#[test]
fn static_route_config_round_trips_as_json() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".to_string(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        tables: vec![TableConfig {
            name: "edge".to_string(),
            nettype: None,
            kernel_table: Some(100),
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            trie: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }],
        protocols: vec![ProtocolConfig::Static {
            name: "static-edge".to_string(),
            table: "edge".to_string(),
            routes: vec![StaticRoute {
                prefix: "203.0.113.0/24".to_string(),
                next_hop: Some("192.0.2.254".to_string()),
                blackhole: false,
                address_family: AddressFamily::Ipv4,
                nexthop_type: Some(StaticNexthopType::Router),
                mpls_label: None,
                igp_metric: None,
            }],
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
                mpls_channel: None,
        }],
        ..RoutePlaneConfig::default()
    };

    let encoded = serde_json::to_string(&config).expect("config serializes");
    let decoded: RoutePlaneConfig = serde_json::from_str(&encoded).expect("config deserializes");

    assert_eq!(decoded.identity.router_id, "192.0.2.1");
    assert_eq!(decoded.protocols.len(), 1);
}

#[test]
fn validation_rejects_protocol_referencing_missing_table() {
    let config = RoutePlaneConfig {
        protocols: vec![ProtocolConfig::Static {
            name: "bad-static".to_string(),
            table: "missing".to_string(),
            routes: Vec::<StaticRoute>::new(),
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
                mpls_channel: None,
        }],
        ..RoutePlaneConfig::default()
    };

    let error = validate_config(&config).expect_err("missing table should fail");

    assert!(error.to_string().contains("missing table"));
}

#[test]
fn validation_warns_when_router_id_is_empty() {
    let config = RoutePlaneConfig::default();
    let report = validate_config(&config).expect("default config is valid");

    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("router-id"))
    );
}

#[test]
fn diff_reports_changed_protocol_count() {
    let running = RoutePlaneConfig::default();
    let candidate = RoutePlaneConfig {
        protocols: vec![ProtocolConfig::Static {
            name: "static-default".to_string(),
            table: "master".to_string(),
            routes: Vec::new(),
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
                mpls_channel: None,
        }],
        ..RoutePlaneConfig::default()
    };

    let diff = ConfigDiff::between(&running, &candidate);

    assert!(diff.changed);
    assert!(diff.summary.contains(&"protocol count: 0 -> 1".to_string()));
}

#[test]
fn store_commits_candidate_to_running_and_records_revision() {
    let mut store = ConfigStore::new(RoutePlaneConfig::default());
    let candidate = RoutePlaneConfig {
        protocols: vec![ProtocolConfig::Static {
            name: "static-default".to_string(),
            table: "master".to_string(),
            routes: Vec::new(),
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
                mpls_channel: None,
        }],
        ..RoutePlaneConfig::default()
    };

    store
        .replace_candidate(candidate.clone())
        .expect("candidate is valid");
    let diff = store.diff();
    assert!(diff.changed);

    let revision = store
        .commit(CommitRequest {
            author: "operator".to_string(),
            note: "add static protocol".to_string(),
        })
        .expect("commit succeeds");

    assert_eq!(revision.id, 1);
    assert_eq!(store.running(), &candidate);
    assert_eq!(store.revisions().len(), 1);
}

#[test]
fn store_rolls_back_to_previous_revision() {
    let mut store = ConfigStore::new(RoutePlaneConfig::default());
    let first = store
        .commit(CommitRequest {
            author: "operator".to_string(),
            note: "initial".to_string(),
        })
        .expect("initial commit succeeds");

    let changed = RoutePlaneConfig {
        protocols: vec![ProtocolConfig::Static {
            name: "static-default".to_string(),
            table: "master".to_string(),
            routes: Vec::new(),
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
                mpls_channel: None,
        }],
        ..RoutePlaneConfig::default()
    };

    store
        .replace_candidate(changed)
        .expect("candidate is valid");
    store
        .commit(CommitRequest {
            author: "operator".to_string(),
            note: "change".to_string(),
        })
        .expect("second commit succeeds");

    let rollback = store
        .rollback(RollbackRequest {
            revision_id: first.id,
            author: "operator".to_string(),
            note: "rollback".to_string(),
        })
        .expect("rollback succeeds");

    assert_eq!(rollback.id, 3);
    assert_eq!(store.running(), &RoutePlaneConfig::default());
}

// ── MPLS Schema Round-trip Tests ─────────────────────────────

#[test]
fn mpls_domain_round_trips_as_json() {
    let domain = MplsDomain {
        name: "main".into(),
        label_ranges: vec![MplsLabelRange {
            low: 100,
            high: 200,
        }],
        label_policy: Some(MplsLabelPolicy::PerPrefix),
        max_label_stack_depth: Some(8),
        sr_enabled: None,
        sr_global_block: None,
        static_bindings: None,
    };

    let encoded = serde_json::to_string(&domain).expect("serializes");
    let decoded: MplsDomain = serde_json::from_str(&encoded).expect("deserializes");

    assert_eq!(decoded.name, "main");
    assert_eq!(decoded.label_ranges.len(), 1);
    assert_eq!(decoded.label_ranges[0].low, 100);
    assert_eq!(decoded.label_ranges[0].high, 200);
    assert!(matches!(decoded.label_policy, Some(MplsLabelPolicy::PerPrefix)));
    assert_eq!(decoded.max_label_stack_depth, Some(8));
}

#[test]
fn mpls_domain_with_multiple_ranges_round_trips() {
    let domain = MplsDomain {
        name: "dual".into(),
        label_ranges: vec![
            MplsLabelRange { low: 100, high: 199 },
            MplsLabelRange { low: 300, high: 399 },
        ],
        label_policy: None,
        max_label_stack_depth: None,
        sr_enabled: None,
        sr_global_block: None,
        static_bindings: Some(vec![MplsStaticBinding {
            prefix: "10.0.0.0/8".into(),
            label: 150,
        }]),
    };

    let encoded = serde_json::to_string(&domain).expect("serializes");
    let decoded: MplsDomain = serde_json::from_str(&encoded).expect("deserializes");

    assert_eq!(decoded.label_ranges.len(), 2);
    assert_eq!(
        decoded.static_bindings.as_ref().unwrap()[0].label,
        150
    );
    assert_eq!(
        decoded.static_bindings.as_ref().unwrap()[0].prefix,
        "10.0.0.0/8"
    );
}

#[test]
fn all_four_mpls_label_policy_variants_round_trip() {
    for (policy, expected_key) in [
        (MplsLabelPolicy::Static, "static"),
        (MplsLabelPolicy::PerPrefix, "per-prefix"),
        (MplsLabelPolicy::Aggregate, "aggregate"),
        (MplsLabelPolicy::Vrf, "vrf"),
    ] {
        let encoded = serde_json::to_string(&policy).expect("serializes");
        assert!(
            encoded.contains(expected_key),
            "expected '{}' in '{}'",
            expected_key,
            encoded
        );
        let _decoded: MplsLabelPolicy =
            serde_json::from_str(&encoded).expect("deserializes");
    }
}

#[test]
fn mpls_table_config_round_trips() {
    let table = MplsTableConfig {
        name: "mpls1".into(),
        domain: "main".into(),
        gc_threshold: Some(1000),
        gc_period_secs: Some(300),
        sorted: Some(true),
        min_settle_time_secs: Some(1),
        max_settle_time_secs: Some(10),
    };

    let encoded = serde_json::to_string(&table).expect("serializes");
    let decoded: MplsTableConfig = serde_json::from_str(&encoded).expect("deserializes");

    assert_eq!(decoded.name, "mpls1");
    assert_eq!(decoded.domain, "main");
    assert_eq!(decoded.gc_threshold, Some(1000));
    assert_eq!(decoded.sorted, Some(true));
}

#[test]
fn full_config_with_mpls_domain_and_table_round_trips() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 16,
                high: 1023,
            }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls-table-1".into(),
            domain: "main".into(),
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let encoded = serde_json::to_string(&config).expect("serializes");
    let decoded: RoutePlaneConfig = serde_json::from_str(&encoded).expect("deserializes");

    let domains = decoded.mpls_domains.expect("mpls_domains present");
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name, "main");
    let tables = decoded.mpls_tables.expect("mpls_tables present");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "mpls-table-1");
}

// ── MPLS Validation Tests ────────────────────────────────────

#[test]
fn validation_rejects_duplicate_mpls_domain_names() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![
            MplsDomain {
                name: "dup".into(),
                label_ranges: vec![MplsLabelRange { low: 16, high: 99 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: None,
                sr_global_block: None,
                static_bindings: None,
            },
            MplsDomain {
                name: "dup".into(),
                label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: None,
                sr_global_block: None,
                static_bindings: None,
            },
        ]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("duplicate domain names should fail");
    assert!(err.to_string().contains("duplicate"));
    assert!(err.to_string().contains("dup"));
}

#[test]
fn validation_rejects_overlapping_label_ranges() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![
                MplsLabelRange { low: 100, high: 200 },
                MplsLabelRange { low: 150, high: 300 },
            ],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("overlapping ranges should fail");
    assert!(err.to_string().contains("overlap"));
}

#[test]
fn validation_accepts_non_overlapping_label_ranges() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![
                MplsLabelRange { low: 100, high: 199 },
                MplsLabelRange { low: 300, high: 399 },
            ],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let report = validate_config(&config).expect("non-overlapping ranges should pass");
    assert!(report.warnings.is_empty() || report.warnings.iter().all(|w| w.contains("router-id")));
}

#[test]
fn validation_rejects_label_below_reserved_range() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 5, high: 100 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("label below 16 should fail");
    assert!(err.to_string().contains("below reserved"));
}

#[test]
fn validation_rejects_label_above_20bit_space() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 1_000_000,
                high: 2_000_000,
            }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("label above 20-bit should fail");
    assert!(err.to_string().contains("exceeds"));
}

#[test]
fn validation_rejects_static_binding_outside_ranges() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: Some(vec![MplsStaticBinding {
                prefix: "10.0.0.0/8".into(),
                label: 999,
            }]),
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("static binding outside ranges should fail");
    assert!(err.to_string().contains("outside configured ranges"));
}

#[test]
fn validation_rejects_duplicate_static_binding_labels() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 299 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: Some(vec![
                MplsStaticBinding {
                    prefix: "10.0.0.0/8".into(),
                    label: 150,
                },
                MplsStaticBinding {
                    prefix: "172.16.0.0/12".into(),
                    label: 150,
                },
            ]),
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("duplicate static binding labels should fail");
    assert!(err.to_string().contains("duplicate static binding"));
}

#[test]
fn validation_rejects_mpls_table_with_non_existent_domain() {
    let config = RoutePlaneConfig {
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls1".into(),
            domain: "ghost".into(),
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("non-existent domain reference should fail");
    assert!(err.to_string().contains("non-existent domain"));
}

#[test]
fn validation_rejects_mpls_channel_with_non_existent_table() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16, high: 99 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        protocols: vec![ProtocolConfig::Static {
            name: "stat".into(),
            table: "master".into(),
            routes: vec![],
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
            mpls_channel: Some(MplsChannelConfig {
                table: "ghost-mpls".into(),
                import_limit: None,
                import_limit_action: None,
                export_limit: None,
                export_limit_action: None,
                import_keep_filtered: None,
            }),
        }],
        ..RoutePlaneConfig::default()
    };

    let err =
        validate_config(&config).expect_err("non-existent MPLS table reference should fail");
    assert!(err.to_string().contains("non-existent MPLS table"));
}

#[test]
fn validation_rejects_invalid_max_label_stack_depth() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16, high: 99 }],
            label_policy: None,
            max_label_stack_depth: Some(0),
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("stack depth 0 should fail");
    assert!(err.to_string().contains("max_label_stack_depth"));
}

#[test]
fn validation_accepts_valid_mpls_config() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
            label_policy: Some(MplsLabelPolicy::PerPrefix),
            max_label_stack_depth: Some(8),
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: Some(vec![MplsStaticBinding {
                prefix: "10.0.0.0/8".into(),
                label: 150,
            }]),
        }]),
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls1".into(),
            domain: "main".into(),
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let report = validate_config(&config).expect("valid MPLS config should pass");
    // Only router-id warning is expected (from default)
    assert!(report.warnings.is_empty() || report.warnings.iter().all(|w| w.contains("router-id")));
}
