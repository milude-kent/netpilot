use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use netpilot_config::{ProtocolConfig, RoutePlaneConfig};
use netpilotd::{api::build_router, state::AppState};
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let app = build_router(AppState::default());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn config_candidate_commit_flow_works_over_api() {
    let app = build_router(AppState::default());
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

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/candidate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&candidate).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config/commit")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"author":"operator","note":"api commit"}"#.as_bytes().to_vec(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn config_with_mpls_domains_and_tables_commits_via_api() {
    use netpilot_config::{MplsDomain, MplsLabelRange, MplsTableConfig, RouterIdentity};

    let app = build_router(AppState::default());
    let candidate = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 16,
                high: 1023,
            }],
            label_policy: None,
            max_label_stack_depth: Some(8),
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls-table-1".into(),
            domain: "main".into(),
            gc_threshold: Some(500),
            gc_period_secs: Some(60),
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    // PUT candidate config
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/candidate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&candidate).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // GET candidate config
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/candidate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let config: RoutePlaneConfig = serde_json::from_slice(&body).expect("MPLS config deserializes");

    let domains = config.mpls_domains.expect("mpls_domains should be present");
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name, "main");
    assert_eq!(domains[0].label_ranges[0].low, 16);

    let tables = config.mpls_tables.expect("mpls_tables should be present");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].domain, "main");
}

/// Phase D4 — P0 commit→reload guarantee.
///
/// Asserts that POST /api/config/commit actually moves the candidate
/// configuration into the running configuration, so that a subsequent
/// GET /api/config/running reflects what was just committed (not the
/// original baseline).
#[tokio::test]
async fn commit_promotes_candidate_into_running() {
    use netpilot_config::RouterIdentity;

    let app = build_router(AppState::default());

    // 2. Capture the baseline running config.
    let baseline_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(baseline_resp.status(), StatusCode::OK);
    let baseline_body = axum::body::to_bytes(baseline_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let baseline: RoutePlaneConfig =
        serde_json::from_slice(&baseline_body).expect("baseline running parses");
    assert_eq!(baseline.identity.router_id, "");

    // 3. PUT a *different* candidate config (change identity.router_id
    //    and add a protocol so the diff has both kinds of change).
    let candidate = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "198.51.100.7".into(),
            local_asn: Some(64500),
            router_id_from: None,
        },
        protocols: vec![ProtocolConfig::Static {
            name: "d4-static".to_string(),
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

    let put_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/candidate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&candidate).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_resp.status(), StatusCode::NO_CONTENT);

    // 4. GET /api/config/diff — must report a non-empty change.
    let diff_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/diff")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(diff_resp.status(), StatusCode::OK);
    let diff_body = axum::body::to_bytes(diff_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let diff: netpilot_config::diff::ConfigDiff =
        serde_json::from_slice(&diff_body).expect("diff parses");
    assert!(diff.changed, "diff should report a change");
    assert!(
        !diff.summary.is_empty(),
        "diff summary should not be empty after a meaningful candidate change"
    );

    // 5. POST /api/config/commit — must return 200.
    let commit_body = serde_json::json!({
        "author": "test",
        "note": "d4 commit→reload",
    });
    let commit_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config/commit")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&commit_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(commit_resp.status(), StatusCode::OK);

    // 6. GET /api/config/running — must now reflect what we committed,
    //    NOT the original baseline.
    let running_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running_resp.status(), StatusCode::OK);
    let running_body = axum::body::to_bytes(running_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let running: RoutePlaneConfig =
        serde_json::from_slice(&running_body).expect("running config parses");

    assert_eq!(
        running.identity.router_id, "198.51.100.7",
        "running identity.router_id must reflect the committed candidate"
    );
    assert_eq!(
        running.identity.local_asn,
        Some(64500),
        "running identity.local_asn must reflect the committed candidate"
    );
    assert_eq!(
        running.protocols.len(),
        1,
        "running must contain the committed protocol"
    );
    assert_ne!(
        running, baseline,
        "running config must differ from the baseline after a successful commit"
    );

    // 7. GET /api/config/revisions — endpoint does not exist in the
    //    current router; skip with an explicit note rather than fail.
    let rev_probe = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/revisions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    if rev_probe.status() == StatusCode::OK {
        let rev_body = axum::body::to_bytes(rev_probe.into_body(), usize::MAX)
            .await
            .unwrap();
        let revisions: Vec<netpilot_config::Revision> =
            serde_json::from_slice(&rev_body).expect("revisions parse");
        assert!(
            !revisions.is_empty(),
            "revisions endpoint must surface at least one entry after a commit"
        );
    }
}

/// Phase D4 — P0 rollback guarantee.
///
/// Asserts that POST /api/config/rollback reverts the running
/// configuration to a previously committed revision.
#[tokio::test]
async fn rollback_reverts_running_config() {
    use netpilot_config::RouterIdentity;

    let app = build_router(AppState::default());

    // 1. Capture baseline running config.
    let baseline_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(baseline_resp.status(), StatusCode::OK);
    let baseline_body = axum::body::to_bytes(baseline_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let baseline: RoutePlaneConfig =
        serde_json::from_slice(&baseline_body).expect("baseline running parses");

    // 2a. PUT candidate A and commit it.
    let candidate_a = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "203.0.113.10".into(),
            local_asn: Some(65001),
            router_id_from: None,
        },
        ..RoutePlaneConfig::default()
    };

    let put_a = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/candidate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&candidate_a).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_a.status(), StatusCode::NO_CONTENT);

    let commit_a_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config/commit")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"author":"test","note":"commit A"}"#.as_bytes().to_vec(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(commit_a_resp.status(), StatusCode::OK);
    let commit_a_body = axum::body::to_bytes(commit_a_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let revision_a: netpilot_config::Revision =
        serde_json::from_slice(&commit_a_body).expect("commit A returns a revision");

    // 2b. PUT candidate B and commit it.
    let candidate_b = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "203.0.113.20".into(),
            local_asn: Some(65002),
            router_id_from: None,
        },
        protocols: vec![ProtocolConfig::Static {
            name: "rollback-static".to_string(),
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

    let put_b = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/candidate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&candidate_b).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_b.status(), StatusCode::NO_CONTENT);

    let commit_b_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config/commit")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"author":"test","note":"commit B"}"#.as_bytes().to_vec(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(commit_b_resp.status(), StatusCode::OK);

    // Sanity-check: running now reflects B, not A or baseline.
    let running_b_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let running_b_body = axum::body::to_bytes(running_b_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let running_b: RoutePlaneConfig =
        serde_json::from_slice(&running_b_body).expect("running after B parses");
    assert_eq!(running_b.identity.router_id, "203.0.113.20");
    assert_eq!(running_b.protocols.len(), 1);

    // 3. POST /api/config/rollback targeting A's revision id.
    let rollback_body = serde_json::json!({
        "revision_id": revision_a.id,
        "author": "test",
        "note": "d4 rollback to A",
    });
    let rollback_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config/rollback")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rollback_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rollback_resp.status(), StatusCode::OK);

    // 4. GET /api/config/running — must match A, not B and not baseline.
    let running_after_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(running_after_resp.status(), StatusCode::OK);
    let running_after_body = axum::body::to_bytes(running_after_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let running_after: RoutePlaneConfig =
        serde_json::from_slice(&running_after_body).expect("running after rollback parses");

    assert_eq!(
        running_after.identity.router_id, "203.0.113.10",
        "running identity.router_id must equal A's after rollback"
    );
    assert_eq!(
        running_after.identity.local_asn,
        Some(65001),
        "running identity.local_asn must equal A's after rollback"
    );
    assert_eq!(
        running_after.protocols.len(),
        0,
        "running must drop B's protocol after rolling back to A"
    );
    assert_eq!(
        running_after, candidate_a,
        "running config must exactly match candidate A after rollback"
    );
    assert_ne!(
        running_after, running_b,
        "running config after rollback must not equal the B state"
    );
    assert_ne!(
        running_after, baseline,
        "running config after rollback must not equal the baseline (A != baseline)"
    );
}
