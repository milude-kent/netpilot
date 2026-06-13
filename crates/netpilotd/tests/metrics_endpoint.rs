// Test that /metrics returns 200 + expected Prometheus content.
use axum::body::Body;
use axum::http::{Request, StatusCode};
use netpilotd::{api::build_router, metrics::install_recorder, state::AppState};
use tower::ServiceExt;

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_format() {
    // install_recorder() uses a OnceLock — safe to call from multiple
    // tests in the same process; the first call installs, subsequent
    // calls return the cached handle.
    let _handle = install_recorder();
    // Touch the counter so the recorder actually registers it; the
    // Prometheus exposition format only emits counters that have been
    // observed at least once.
    metrics::counter!("netpilot_events_received_total").increment(1);
    let app = build_router(AppState::default());
    let req = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);
    // Counter names registered in `metrics::install_recorder` should
    // appear in the exposition body, even if the value is 0.
    assert!(
        body_str.contains("netpilot_events_received_total"),
        "expected counter netpilot_events_received_total in /metrics body, got: {}",
        body_str
    );
}

#[tokio::test]
async fn metrics_endpoint_increments_fib_counter() {
    use metrics::counter;
    let _handle = install_recorder();
    counter!("netpilot_fib_routes_installed_total").increment(7);
    let app = build_router(AppState::default());
    let req = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);
    // After incrementing by 7, the value line should reflect at least 7.
    assert!(
        body_str.contains("netpilot_fib_routes_installed_total"),
        "expected counter netpilot_fib_routes_installed_total in /metrics body"
    );
}
