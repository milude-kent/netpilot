use crate::state::AppState;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use routeplane_config::{CommitRequest, RollbackRequest, RoutePlaneConfig};
use serde::Deserialize;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/config/running", get(get_running_config))
        .route(
            "/api/config/candidate",
            get(get_candidate_config).put(put_candidate_config),
        )
        .route("/api/config/diff", get(get_config_diff))
        .route("/api/config/commit", post(commit_config))
        .route("/api/config/rollback", post(rollback_config))
        .with_state(state)
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

async fn get_config_diff(
    State(state): State<AppState>,
) -> Json<routeplane_config::diff::ConfigDiff> {
    let store = state.config_store.read().await;
    Json(store.diff())
}

async fn commit_config(
    State(state): State<AppState>,
    Json(request): Json<ApiCommitRequest>,
) -> Result<Json<routeplane_config::Revision>, (StatusCode, String)> {
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
) -> Result<Json<routeplane_config::Revision>, (StatusCode, String)> {
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
