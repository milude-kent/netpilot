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
