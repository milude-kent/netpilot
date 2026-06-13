use crate::auth_mw;
use crate::state::AppState;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use netpilot_config::{CommitRequest, RollbackRequest, RoutePlaneConfig};
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tower_http::services::ServeDir;
use tracing::warn;

pub fn build_router(state: AppState) -> Router {
    // Allowlisted routes that bypass the bearer auth middleware. They
    // share the same Router as the protected routes so they can be served
    // from the same listener; the middleware is applied via
    // `route_layer` only on the protected sub-router below.
    let allowlisted = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics_handler));

    // Protected sub-router: every API route runs through the bearer
    // auth middleware, including `/api/events` (SSE), which accepts
    // the token via `?token=...` as well as the `Authorization: Bearer`
    // header.
    let protected = Router::new()
        .route("/api/config/running", get(get_running_config))
        .route(
            "/api/config/candidate",
            get(get_candidate_config).put(put_candidate_config),
        )
        .route("/api/config/diff", get(get_config_diff))
        .route("/api/config/commit", post(commit_config))
        .route("/api/config/commit-confirmed", post(commit_confirmed))
        .route("/api/config/confirm", post(confirm_config))
        .route("/api/config/undo", post(undo_config))
        .route("/api/config/rollback", post(rollback_config))
        .route("/api/events", get(sse_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_mw::bearer_auth_middleware,
        ));

    Router::new()
        .merge(allowlisted)
        .merge(protected)
        .with_state(state)
        .fallback_service(ServeDir::new("crates/netpilot-web/dist"))
}

async fn health() -> &'static str {
    "ok"
}

async fn get_running_config(State(state): State<AppState>) -> Json<RoutePlaneConfig> {
    let store = state.config_store.read().await;
    Json(store.running().clone())
}

async fn get_candidate_config(State(state): State<AppState>) -> Json<RoutePlaneConfig> {
    let store = state.config_store.read().await;
    Json(store.candidate().clone())
}

async fn put_candidate_config(
    State(state): State<AppState>,
    Json(candidate): Json<RoutePlaneConfig>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut store = state.config_store.write().await;
    store
        .replace_candidate(candidate)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_config_diff(State(state): State<AppState>) -> Json<netpilot_config::diff::ConfigDiff> {
    let store = state.config_store.read().await;
    Json(store.diff())
}

async fn commit_config(
    State(state): State<AppState>,
    Json(request): Json<ApiCommitRequest>,
) -> Result<Json<netpilot_config::Revision>, (StatusCode, String)> {
    let mut store = state.config_store.write().await;
    let revision = store
        .commit(CommitRequest {
            author: request.author,
            note: request.note,
        })
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(revision))
}

async fn rollback_config(
    State(state): State<AppState>,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<netpilot_config::Revision>, (StatusCode, String)> {
    let mut store = state.config_store.write().await;
    let revision = store
        .rollback(request)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(revision))
}

#[derive(Deserialize)]
struct ApiCommitRequest {
    author: String,
    note: String,
}

#[derive(Deserialize)]
struct ApiConfirmCommitRequest {
    author: String,
    note: String,
    timeout_secs: u32,
}

async fn commit_confirmed(
    State(state): State<AppState>,
    Json(request): Json<ApiConfirmCommitRequest>,
) -> Result<Json<netpilot_config::Revision>, (StatusCode, String)> {
    let revision = state
        .scheduler
        .commit_with_timeout(
            CommitRequest {
                author: request.author,
                note: request.note,
            },
            request.timeout_secs,
        )
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(revision))
}

async fn confirm_config(State(state): State<AppState>) -> Result<StatusCode, (StatusCode, String)> {
    state
        .scheduler
        .confirm()
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn undo_config(
    State(state): State<AppState>,
) -> Result<Json<netpilot_config::Revision>, (StatusCode, String)> {
    let mut store = state.config_store.write().await;
    let revision = store
        .undo()
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(revision))
}

async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx);
    Sse::new(stream.map(|result| match result {
        Ok(event) => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            Ok(Event::default().event("protocol").data(json))
        }
        Err(BroadcastStreamRecvError::Lagged(skipped)) => {
            // The broadcast channel overran the slow consumer. Surface
            // this to the client as a structured event so it knows it
            // missed data, and bump the counter for ops visibility.
            metrics::counter!("netpilot_events_lagged_total").increment(1);
            warn!(skipped, "SSE subscriber lagged behind broadcast channel");
            let payload = serde_json::json!({
                "type": "lagged",
                "skipped": skipped,
            })
            .to_string();
            Ok(Event::default().event("lagged").data(payload))
        }
    }))
}

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    (
        [("content-type", "text/plain; version=0.0.4")],
        state.metrics.render(),
    )
}
