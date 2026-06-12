# RoutePlane Milestone 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working RoutePlane foundation: a Rust workspace with `routeplaned`, structured configuration types, candidate/running commit and rollback logic, a minimal REST API, and tests proving the configuration workflow.

**Architecture:** Start with an all-in-one `routeplaned` binary and focused crates. `routeplane-config` owns schema, validation, revisions, diffs, commit, and rollback. `routeplaned` exposes a small API over that service. Protocol actors, RIB, policy VM, and kernel netlink are represented only by clean module boundaries in this milestone.

**Tech Stack:** Rust 1.95, Cargo workspace, `tokio`, `axum`, `serde`, `serde_json`, `thiserror`, `time`, `tempfile`, and `reqwest` for API tests.

---

## File Structure

- Create: `Cargo.toml`
  - Workspace manifest for the first two crates.
- Create: `crates/routeplane-config/Cargo.toml`
  - Library crate manifest for structured configuration and commit logic.
- Create: `crates/routeplane-config/src/lib.rs`
  - Public exports for config, store, revision, diff, and validation modules.
- Create: `crates/routeplane-config/src/schema.rs`
  - Versioned structured configuration types.
- Create: `crates/routeplane-config/src/diff.rs`
  - Lightweight structured diff summary between candidate and running config.
- Create: `crates/routeplane-config/src/validation.rs`
  - Semantic validation for router-id, tables, protocols, and dangerous changes.
- Create: `crates/routeplane-config/src/store.rs`
  - In-memory candidate/running store with revision history and rollback.
- Create: `crates/routeplane-config/tests/config_store.rs`
  - Integration tests for candidate edit, commit, diff, validation, and rollback.
- Create: `crates/routeplaned/Cargo.toml`
  - Binary crate manifest for the daemon and API.
- Create: `crates/routeplaned/src/main.rs`
  - Process entry point.
- Create: `crates/routeplaned/src/api.rs`
  - Axum routes for health, config, diff, commit, rollback, and revisions.
- Create: `crates/routeplaned/src/state.rs`
  - Shared application state wrapper around the config store.
- Create: `crates/routeplaned/tests/api_config.rs`
  - API integration tests.
- Create: `README.md`
  - Project name, scope, and first-run commands.

## Task 1: Workspace and Project Identity

**Files:**
- Create: `Cargo.toml`
- Create: `README.md`
- Modify: `docs/superpowers/specs/2026-06-12-routeplane-routing-platform-design.md`

- [ ] **Step 1: Create the workspace manifest**

```toml
[workspace]
members = [
    "crates/routeplane-config",
]
resolver = "2"

[workspace.package]
edition = "2024"
license = "Apache-2.0"

[workspace.dependencies]
axum = "0.8"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
thiserror = "2"
time = { version = "0.3", features = ["formatting", "macros", "serde"] }
tokio = { version = "1", features = ["macros", "net", "rt-multi-thread", "signal", "sync"] }
tower = { version = "0.5", features = ["util"] }
```

- [ ] **Step 2: Create the README**

```markdown
# RoutePlane

RoutePlane is a Rust routing platform inspired by BIRD2. The first milestone builds the daemon foundation, structured configuration model, candidate/running commit workflow, rollback support, and a small REST API.

The long-term architecture is a Rust microkernel with protocol actors for BGP, OSPF, RIP, Babel, Static, Direct, Kernel, BFD, RPKI, MRT, and Pipe.

## First Milestone

- `routeplane-config`: structured config schema, validation, diff, revision history, commit, and rollback.
- `routeplaned`: all-in-one daemon with a REST API for health and configuration workflow.

## Development

```powershell
cargo test
cargo run -p routeplaned
```
```

- [ ] **Step 3: Run formatting check**

Run: `cargo fmt --check`

Expected: Cargo reports no Rust packages yet or formatting succeeds after crate files exist. If Cargo reports missing package manifests before Task 2, continue to Task 2 and rerun formatting there.

- [ ] **Step 4: Commit**

```powershell
git add Cargo.toml README.md docs/superpowers/specs/2026-06-12-routeplane-routing-platform-design.md
git commit -m "chore: name project RoutePlane"
```

## Task 2: Configuration Schema

**Files:**
- Create: `crates/routeplane-config/Cargo.toml`
- Create: `crates/routeplane-config/src/lib.rs`
- Create: `crates/routeplane-config/src/schema.rs`
- Create: `crates/routeplane-config/tests/config_store.rs`

- [ ] **Step 1: Write failing schema tests**

```rust
use routeplane_config::{
    AddressFamily, ProtocolConfig, RoutePlaneConfig, RouterIdentity, StaticRoute, TableConfig,
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
            }],
        }],
        ..RoutePlaneConfig::default()
    };

    let encoded = serde_json::to_string(&config).expect("config serializes");
    let decoded: RoutePlaneConfig = serde_json::from_str(&encoded).expect("config deserializes");

    assert_eq!(decoded.identity.router_id, "192.0.2.1");
    assert_eq!(decoded.protocols.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p routeplane-config schema`

Expected: FAIL because the crate and types do not exist yet.

- [ ] **Step 3: Add the crate manifest**

```toml
[package]
name = "routeplane-config"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
time.workspace = true
```

- [ ] **Step 4: Add public exports**

```rust
pub mod diff;
pub mod schema;
pub mod store;
pub mod validation;

pub use schema::{
    AddressFamily, BgpNeighbor, ProtocolConfig, RoutePlaneConfig, RouterIdentity, StaticRoute,
    TableConfig,
};
pub use store::{CommitRequest, ConfigStore, Revision, RollbackRequest};
pub use validation::{ValidationError, ValidationReport};
```

- [ ] **Step 5: Add the schema implementation**

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RoutePlaneConfig {
    pub schema_version: u32,
    pub identity: RouterIdentity,
    pub tables: Vec<TableConfig>,
    pub protocols: Vec<ProtocolConfig>,
}

impl Default for RoutePlaneConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            identity: RouterIdentity::default(),
            tables: vec![TableConfig {
                name: "master".to_string(),
                kernel_table: Some(254),
            }],
            protocols: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouterIdentity {
    pub router_id: String,
    pub local_asn: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TableConfig {
    pub name: String,
    pub kernel_table: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProtocolConfig {
    Static {
        name: String,
        table: String,
        routes: Vec<StaticRoute>,
    },
    Bgp {
        name: String,
        table: String,
        local_asn: u32,
        neighbors: Vec<BgpNeighbor>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StaticRoute {
    pub prefix: String,
    pub next_hop: Option<String>,
    pub blackhole: bool,
    pub address_family: AddressFamily,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BgpNeighbor {
    pub name: String,
    pub remote_address: String,
    pub remote_asn: u32,
    pub address_families: Vec<AddressFamily>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}
```

- [ ] **Step 6: Add empty module files so exports compile**

Create `crates/routeplane-config/src/diff.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub changed: bool,
    pub summary: Vec<String>,
}
```

Create `crates/routeplane-config/src/validation.rs`:

```rust
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("{0}")]
    Message(String),
}
```

Create `crates/routeplane-config/src/store.rs`:

```rust
use crate::schema::RoutePlaneConfig;

#[derive(Clone, Debug)]
pub struct CommitRequest {
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct RollbackRequest {
    pub revision_id: u64,
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Revision {
    pub id: u64,
    pub config: RoutePlaneConfig,
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    running: RoutePlaneConfig,
    candidate: RoutePlaneConfig,
    revisions: Vec<Revision>,
}
```

- [ ] **Step 7: Run tests to verify schema passes**

Run: `cargo test -p routeplane-config`

Expected: PASS for schema tests. Warnings about unused fields in `ConfigStore` are acceptable until Task 4.

- [ ] **Step 8: Commit**

```powershell
git add crates/routeplane-config
git commit -m "feat: add RoutePlane config schema"
```

## Task 3: Validation and Diff

**Files:**
- Modify: `crates/routeplane-config/src/diff.rs`
- Modify: `crates/routeplane-config/src/validation.rs`
- Modify: `crates/routeplane-config/tests/config_store.rs`

- [ ] **Step 1: Add failing validation and diff tests**

```rust
use routeplane_config::{
    diff::ConfigDiff, validation::validate_config, ProtocolConfig, RoutePlaneConfig, StaticRoute,
};

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

    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("router-id")));
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p routeplane-config`

Expected: FAIL because `validate_config` and `ConfigDiff::between` do not exist.

- [ ] **Step 3: Implement diff**

```rust
use crate::schema::RoutePlaneConfig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub changed: bool,
    pub summary: Vec<String>,
}

impl ConfigDiff {
    pub fn between(running: &RoutePlaneConfig, candidate: &RoutePlaneConfig) -> Self {
        let mut summary = Vec::new();

        if running.identity != candidate.identity {
            summary.push("identity changed".to_string());
        }

        if running.tables.len() != candidate.tables.len() {
            summary.push(format!(
                "table count: {} -> {}",
                running.tables.len(),
                candidate.tables.len()
            ));
        }

        if running.protocols.len() != candidate.protocols.len() {
            summary.push(format!(
                "protocol count: {} -> {}",
                running.protocols.len(),
                candidate.protocols.len()
            ));
        }

        if running != candidate && summary.is_empty() {
            summary.push("configuration changed".to_string());
        }

        Self {
            changed: running != candidate,
            summary,
        }
    }
}
```

- [ ] **Step 4: Implement validation**

```rust
use crate::schema::{ProtocolConfig, RoutePlaneConfig};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("{0}")]
    Message(String),
}

pub fn validate_config(config: &RoutePlaneConfig) -> Result<ValidationReport, ValidationError> {
    let mut warnings = Vec::new();

    if config.schema_version != 1 {
        return Err(ValidationError::Message(format!(
            "unsupported schema version {}",
            config.schema_version
        )));
    }

    if config.identity.router_id.trim().is_empty() {
        warnings.push("router-id is empty; dynamic protocols may require it".to_string());
    }

    let table_names = config
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .collect::<HashSet<_>>();

    for protocol in &config.protocols {
        let table = match protocol {
            ProtocolConfig::Static { table, .. } => table,
            ProtocolConfig::Bgp { table, .. } => table,
        };

        if !table_names.contains(table.as_str()) {
            return Err(ValidationError::Message(format!(
                "protocol references missing table '{table}'"
            )));
        }
    }

    Ok(ValidationReport { warnings })
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p routeplane-config`

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/routeplane-config
git commit -m "feat: validate and diff RoutePlane config"
```

## Task 4: Candidate, Commit, Revision, and Rollback Store

**Files:**
- Modify: `crates/routeplane-config/src/store.rs`
- Modify: `crates/routeplane-config/src/lib.rs`
- Modify: `crates/routeplane-config/tests/config_store.rs`

- [ ] **Step 1: Add failing store workflow tests**

```rust
use routeplane_config::{CommitRequest, ConfigStore, ProtocolConfig, RoutePlaneConfig, RollbackRequest};

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

    store.replace_candidate(candidate.clone()).expect("candidate is valid");
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

    store.replace_candidate(changed).expect("candidate is valid");
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p routeplane-config store`

Expected: FAIL because store methods are not implemented.

- [ ] **Step 3: Implement store workflow**

```rust
use crate::{
    diff::ConfigDiff,
    schema::RoutePlaneConfig,
    validation::{validate_config, ValidationError},
};
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct CommitRequest {
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct RollbackRequest {
    pub revision_id: u64,
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Revision {
    pub id: u64,
    pub config: RoutePlaneConfig,
    pub author: String,
    pub note: String,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    running: RoutePlaneConfig,
    candidate: RoutePlaneConfig,
    revisions: Vec<Revision>,
    next_revision_id: u64,
}

impl ConfigStore {
    pub fn new(initial: RoutePlaneConfig) -> Self {
        Self {
            running: initial.clone(),
            candidate: initial,
            revisions: Vec::new(),
            next_revision_id: 1,
        }
    }

    pub fn running(&self) -> &RoutePlaneConfig {
        &self.running
    }

    pub fn candidate(&self) -> &RoutePlaneConfig {
        &self.candidate
    }

    pub fn revisions(&self) -> &[Revision] {
        &self.revisions
    }

    pub fn replace_candidate(&mut self, candidate: RoutePlaneConfig) -> Result<(), ValidationError> {
        validate_config(&candidate)?;
        self.candidate = candidate;
        Ok(())
    }

    pub fn diff(&self) -> ConfigDiff {
        ConfigDiff::between(&self.running, &self.candidate)
    }

    pub fn commit(&mut self, request: CommitRequest) -> Result<Revision, ValidationError> {
        validate_config(&self.candidate)?;
        self.running = self.candidate.clone();
        let revision = self.create_revision(request.author, request.note, self.running.clone());
        self.revisions.push(revision.clone());
        Ok(revision)
    }

    pub fn rollback(&mut self, request: RollbackRequest) -> Result<Revision, ValidationError> {
        let target = self
            .revisions
            .iter()
            .find(|revision| revision.id == request.revision_id)
            .map(|revision| revision.config.clone())
            .ok_or_else(|| {
                ValidationError::Message(format!(
                    "revision {} does not exist",
                    request.revision_id
                ))
            })?;

        validate_config(&target)?;
        self.running = target.clone();
        self.candidate = target.clone();
        let revision = self.create_revision(request.author, request.note, target);
        self.revisions.push(revision.clone());
        Ok(revision)
    }

    fn create_revision(&mut self, author: String, note: String, config: RoutePlaneConfig) -> Revision {
        let revision = Revision {
            id: self.next_revision_id,
            config,
            author,
            note,
            created_at: OffsetDateTime::now_utc(),
        };
        self.next_revision_id += 1;
        revision
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p routeplane-config`

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/routeplane-config
git commit -m "feat: add config commit and rollback store"
```

## Task 5: routeplaned API Skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/routeplaned/Cargo.toml`
- Create: `crates/routeplaned/src/main.rs`
- Create: `crates/routeplaned/src/api.rs`
- Create: `crates/routeplaned/src/state.rs`
- Create: `crates/routeplaned/tests/api_config.rs`

- [ ] **Step 1: Add failing API tests**

```rust
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use routeplaned::{api::build_router, state::AppState};
use routeplane_config::{ProtocolConfig, RoutePlaneConfig};
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let app = build_router(AppState::default());
    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p routeplaned`

Expected: FAIL because `routeplaned` crate does not exist.

- [ ] **Step 3: Add routeplaned manifest**

Update the workspace manifest to include the daemon crate:

```toml
[workspace]
members = [
    "crates/routeplane-config",
    "crates/routeplaned",
]
resolver = "2"
```

Then add the crate manifest:

```toml
[package]
name = "routeplaned"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
axum.workspace = true
routeplane-config = { path = "../routeplane-config" }
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true

[dev-dependencies]
tower.workspace = true
```

- [ ] **Step 4: Add app state**

```rust
use routeplane_config::{ConfigStore, RoutePlaneConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_store: Arc<RwLock<ConfigStore>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config_store: Arc::new(RwLock::new(ConfigStore::new(RoutePlaneConfig::default()))),
        }
    }
}
```

- [ ] **Step 5: Add API routes**

```rust
use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use routeplane_config::{CommitRequest, RoutePlaneConfig, RollbackRequest};
use serde::Deserialize;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/config/running", get(get_running_config))
        .route("/api/config/candidate", get(get_candidate_config).put(put_candidate_config))
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

async fn get_config_diff(State(state): State<AppState>) -> Json<routeplane_config::diff::ConfigDiff> {
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
```

- [ ] **Step 6: Add lib exports and main**

Create `crates/routeplaned/src/lib.rs`:

```rust
pub mod api;
pub mod state;
```

Create `crates/routeplaned/src/main.rs`:

```rust
use routeplaned::{api::build_router, state::AppState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = build_router(AppState::default());
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 7: Derive serialization for revision request/response**

Update `crates/routeplane-config/src/store.rs` so `CommitRequest`, `RollbackRequest`, and `Revision` derive serde traits:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommitRequest {
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RollbackRequest {
    pub revision_id: u64,
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Revision {
    pub id: u64,
    pub config: RoutePlaneConfig,
    pub author: String,
    pub note: String,
    pub created_at: OffsetDateTime,
}
```

- [ ] **Step 8: Run API tests**

Run: `cargo test -p routeplaned`

Expected: PASS.

- [ ] **Step 9: Commit**

```powershell
git add crates/routeplaned crates/routeplane-config
git commit -m "feat: expose config workflow API"
```

## Task 6: Formatting, Test Suite, and Developer Baseline

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add verification commands to README**

```markdown
## Verification

```powershell
cargo fmt --check
cargo test
cargo run -p routeplaned
```

The API listens on `127.0.0.1:8080` in the first milestone.
```

- [ ] **Step 2: Run formatter**

Run: `cargo fmt`

Expected: Files are formatted without errors.

- [ ] **Step 3: Run formatting check**

Run: `cargo fmt --check`

Expected: PASS.

- [ ] **Step 4: Run full test suite**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 5: Run daemon smoke check**

Run: `cargo run -p routeplaned`

Expected: The process starts and binds `127.0.0.1:8080`. Stop it with `Ctrl+C` after confirming startup. If port `8080` is already used, change the bind address in a later task; do not broaden scope in this milestone.

- [ ] **Step 6: Commit**

```powershell
git add README.md Cargo.toml crates
git commit -m "test: verify RoutePlane milestone 1 baseline"
```

## Self-Review

Spec coverage for this milestone:

- Project name: covered by Task 1.
- Rust workspace and `routeplaned`: covered by Tasks 1 and 5.
- Candidate/running workflow: covered by Task 4.
- Commit and rollback: covered by Task 4 and API tests in Task 5.
- Structured config schema: covered by Task 2.
- Validation and diff: covered by Task 3.
- Web/API foundation: covered by Task 5 as REST API skeleton.
- Protocol actor/RIB/kernel/policy details: intentionally deferred beyond Milestone 1, matching the design's recommended build order.

No dynamic plugin loading, multi-node controller orchestration, full protocol implementation, or Web frontend is included in this milestone.
