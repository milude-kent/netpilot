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
