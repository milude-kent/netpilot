//! Tests for the C1 bearer-auth middleware on the REST control plane.
//!
//! The tests exercise the same oneshot pattern used by
//! `tests/api_config.rs` and `tests/metrics_endpoint.rs`. They install
//! a known bearer secret on the `AppState` and synthesize a valid token
//! using the same format the middleware expects: `<exp_unix>.<hex_hmac>`.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use netpilot_config::AuthConfig;
use netpilotd::{api::build_router, auth_mw::generate_bearer_token, state::AppState};
use tower::ServiceExt;

const SECRET: &str = "test-secret-do-not-use-in-prod";

fn app_with_auth() -> AppState {
    let auth = AuthConfig {
        bearer_secret: Some(SECRET.to_string()),
        // Keep the default unauthed_paths so /health and /metrics pass
        // through untouched.
        ..AuthConfig::default()
    };
    AppState::with_metrics_and_auth(netpilotd::metrics::install_recorder(), auth)
}

#[tokio::test]
async fn health_is_allowlisted_and_returns_ok_without_auth() {
    let app = build_router(app_with_auth());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_is_allowlisted_and_returns_ok_without_auth() {
    let app = build_router(app_with_auth());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn running_config_requires_bearer() {
    let app = build_router(app_with_auth());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn running_config_rejects_wrong_bearer() {
    let app = build_router(app_with_auth());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .header("authorization", "Bearer 123.aaaa")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn running_config_accepts_valid_bearer() {
    let app = build_router(app_with_auth());
    let token = generate_bearer_token(SECRET, 60).expect("token");
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn running_config_rejects_expired_bearer() {
    let app = build_router(app_with_auth());
    let token = generate_bearer_token(SECRET, -10).expect("token");
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sse_endpoint_accepts_token_via_query_string() {
    let app = build_router(app_with_auth());
    let token = generate_bearer_token(SECRET, 60).expect("token");
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/events?token={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn sse_endpoint_rejects_when_query_token_invalid() {
    let app = build_router(app_with_auth());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/events?token=not-a-real-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn no_auth_config_lets_through_everything() {
    // AppState::default() has no bearer_secret, so the middleware should
    // fall through and serve the request normally.
    let app = build_router(AppState::default());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/config/running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
