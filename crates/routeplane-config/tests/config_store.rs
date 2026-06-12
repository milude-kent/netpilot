use routeplane_config::{
    AddressFamily, CommitRequest, ConfigStore, ProtocolConfig, RollbackRequest, RoutePlaneConfig,
    RouterIdentity, StaticNexthopType, StaticRoute, TableConfig, diff::ConfigDiff,
    validation::validate_config,
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
        },
        tables: vec![TableConfig {
            name: "edge".to_string(),
            nettype: None,
            kernel_table: Some(100),
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
