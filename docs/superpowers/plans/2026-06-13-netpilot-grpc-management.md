# NetPilot M6 — gRPC & gNMI Management Interface Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development

**Goal:** Build gRPC + gNMI management interface with tonic: full protobuf service definitions, path-based state access (config, health, protocols, MPLS, SR), ON_CHANGE/STREAM telemetry, and NetPilot native Config/Health RPCs.

**Architecture:** New `netpilot-grpc` crate with tonic server. netpilotd shares AppState via Arc. Proto files compiled via tonic-build. gNMI paths resolve to AppState fields.

**Tech Stack:** tonic 0.12, prost 0.13, prost-types 0.13, tonic-build 0.12

---

### Task 1: New Crate Scaffold

**Files:**
- Create: `crates/netpilot-grpc/Cargo.toml`
- Create: `crates/netpilot-grpc/build.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

Write `crates/netpilot-grpc/Cargo.toml`:
```toml
[package]
name = "netpilot-grpc"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
netpilot-config = { path = "../netpilot-config" }
prost = "0.13"
prost-types = "0.13"
serde_json.workspace = true
tokio.workspace = true
tonic = "0.12"

[build-dependencies]
tonic-build = "0.12"
```

- [ ] **Step 2: Create build.rs**

Write `crates/netpilot-grpc/build.rs`:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .compile_protos(
            &["proto/gnmi.proto", "proto/netpilot.proto"],
            &["proto"],
        )?;
    Ok(())
}
```

- [ ] **Step 3: Add to workspace**

In root `Cargo.toml`, add to members:
```toml
members = [
    "crates/netpilot-config",
    "crates/netpilot-filter",
    "crates/netpilot-grpc",
    "crates/netpilotd",
]
```

- [ ] **Step 4: Build check**

Run: `cargo build -p netpilot-grpc 2>&1`
Expected: PASS (fails with "proto file not found" — expected, proto not created yet)

- [ ] **Step 5: Commit**

```bash
git add crates/netpilot-grpc/Cargo.toml crates/netpilot-grpc/build.rs Cargo.toml
git commit -m "feat: scaffold netpilot-grpc crate with tonic/prost deps

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Proto Definitions

**Files:**
- Create: `crates/netpilot-grpc/proto/gnmi.proto`
- Create: `crates/netpilot-grpc/proto/netpilot.proto`

- [ ] **Step 1: Write gnmi.proto**

Write `crates/netpilot-grpc/proto/gnmi.proto`:
```protobuf
syntax = "proto3";
package gnmi.v1;

service gNMI {
  rpc Capabilities(CapabilityRequest) returns (CapabilityResponse);
  rpc Get(GetRequest) returns (GetResponse);
  rpc Set(SetRequest) returns (SetResponse);
  rpc Subscribe(stream SubscribeRequest) returns (stream SubscribeResponse);
}

message Path {
  repeated string elem = 1;
}

message PathValue {
  Path path = 1;
  bytes value = 2;
}

message CapabilityRequest {}
message CapabilityResponse {
  repeated string supported_models = 1;
  repeated string supported_encodings = 2;
  string version = 3;
}

message GetRequest {
  repeated Path path = 1;
  enum Encoding { JSON = 0; PROTO = 1; }
  Encoding encoding = 2;
}
message GetResponse { repeated PathValue value = 1; }

message SetRequest {
  repeated PathValue replace = 1;
  repeated PathValue update = 2;
  repeated Path delete = 3;
}
message SetResponse {
  repeated PathValue value = 1;
  string message = 2;
}

message SubscribeRequest {
  oneof request {
    SubscriptionList subscribe = 1;
  }
}
message SubscriptionList {
  repeated Subscription subscription = 1;
  enum Mode { STREAM = 0; ONCE = 1; POLL = 2; }
  Mode mode = 1;
}
message Subscription {
  Path path = 1;
  enum Mode { SAMPLE = 0; ON_CHANGE = 1; }
  Mode mode = 1;
  uint64 sample_interval_nanos = 2;
}
message SubscribeResponse {
  oneof response {
    PathValue update = 1;
    bool sync_response = 2;
  }
}
```

- [ ] **Step 2: Write netpilot.proto**

Write `crates/netpilot-grpc/proto/netpilot.proto`:
```protobuf
syntax = "proto3";
package netpilot.v1;

service Config {
  rpc GetRunning(GetRunningRequest) returns (GetRunningResponse);
  rpc PutCandidate(PutCandidateRequest) returns (PutCandidateResponse);
  rpc CommitConfig(CommitConfigRequest) returns (CommitConfigResponse);
  rpc Rollback(RollbackRequest) returns (RollbackResponse);
}

message GetRunningRequest {}
message GetRunningResponse {
  bytes config_json = 1;
  uint64 revision_id = 2;
}

message PutCandidateRequest { bytes config_json = 1; }
message PutCandidateResponse { string status = 1; }

message CommitConfigRequest { string author = 1; string note = 2; }
message CommitConfigResponse { uint64 revision_id = 1; string message = 2; }

message RollbackRequest { uint64 revision_id = 1; string author = 2; string note = 3; }
message RollbackResponse { uint64 revision_id = 1; string message = 2; }

service Health {
  rpc Check(HealthCheckRequest) returns (HealthCheckResponse);
  rpc Watch(HealthCheckRequest) returns (stream HealthCheckResponse);
}

message HealthCheckRequest {}
message HealthCheckResponse {
  string status = 1;
  string uptime_secs = 2;
  uint32 config_revision = 3;
}
```

- [ ] **Step 3: Build check**

Run: `cargo build -p netpilot-grpc 2>&1`
Expected: PASS (proto compiles, generates Rust code in OUT_DIR)

- [ ] **Step 4: Commit**

```bash
git add crates/netpilot-grpc/proto/
git commit -m "feat: add gNMI and NetPilot protobuf service definitions

gnmi.proto: gNMI v1 spec with Capabilities/Get/Set/Subscribe.
netpilot.proto: Config service (GetRunning/PutCandidate/Commit/Rollback)
and Health service (Check/Watch streaming).

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: gRPC Service Implementation

**Files:**
- Create: `crates/netpilot-grpc/src/lib.rs`
- Create: `crates/netpilot-grpc/src/gnmi.rs`
- Create: `crates/netpilot-grpc/src/netpilot_svc.rs`
- Create: `crates/netpilot-grpc/src/path_resolver.rs`

- [ ] **Step 1: Write lib.rs**

Write `crates/netpilot-grpc/src/lib.rs`:
```rust
pub mod gnmi;
pub mod netpilot_svc;
pub mod path_resolver;

use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::transport::Server;

/// Application state shared between gRPC services and the REST API.
/// netpilotd provides this via Arc.
#[derive(Clone)]
pub struct GrpcAppState {
    pub config_store: Arc<RwLock<netpilot_config::ConfigStore>>,
    pub mpls_labels: Arc<RwLock<netpilotd_state::MplsLabelState>>,
    pub sid_registry: Arc<RwLock<netpilotd_sr::SidRegistry>>,
}

/// Build a tonic gRPC server bound to `addr`.
pub fn build_grpc_server(
    addr: std::net::SocketAddr,
    state: GrpcAppState,
) -> impl std::future::Future<Output = Result<(), tonic::transport::Error>> {
    let gnmi_service = gnmi::GnmiService::new(state.clone());
    let config_service = netpilot_svc::ConfigService::new(state.clone());
    let health_service = netpilot_svc::HealthService::new(state);

    Server::builder()
        .add_service(gnmi_service.into_gnmi_server())
        .add_service(config_service.into_config_server())
        .add_service(health_service.into_health_server())
        .serve(addr)
}

// Generated proto code
pub mod proto {
    tonic::include_proto!("gnmi.v1");
    tonic::include_proto!("netpilot.v1");
}

// Minimal type aliases to avoid netpilotd dependency cycles
pub mod netpilotd_state {
    pub use ::std::collections::HashMap;
    pub use ::std::vec::Vec;
    // Placeholder — real types wired at integration time
    #[derive(Clone, Debug, Default)]
    pub struct MplsLabelState {
        pub pools: HashMap<String, ()>,
        pub bindings: Vec<()>,
    }
    impl MplsLabelState {
        pub fn from_domains(_domains: &[netpilot_config::MplsDomain]) -> Self { Self::default() }
    }
}

pub mod netpilotd_sr {
    #[derive(Clone, Debug, Default)]
    pub struct SidRegistry {
        pub prefix_sids: Vec<()>,
        pub adjacency_sids: Vec<()>,
    }
    impl SidRegistry {
        pub fn from_config(_config: &netpilot_config::RoutePlaneConfig) -> Self { Self::default() }
        pub fn list_prefix_sids(&self) -> &[()] { &[] }
        pub fn resolve_prefix_sid(&self, _prefix: &str) -> Option<u32> { None }
    }
}
```

Note: The `netpilotd_state` and `netpilotd_sr` modules are placeholders. When netpilot-grpc is integrated into netpilotd, the real types from netpilotd's mpls and sr modules will be used directly. The placeholder approach avoids a circular dependency between netpilot-grpc and netpilotd.

- [ ] **Step 2: Write gnmi.rs**

Write `crates/netpilot-grpc/src/gnmi.rs`:
```rust
use crate::proto::gnmi::v1::{
    gnmi_server::{Gnmi, GnmiServer},
    CapabilityRequest, CapabilityResponse, GetRequest, GetResponse, PathValue,
    SetRequest, SetResponse, SubscribeRequest, SubscribeResponse, SubscriptionList,
};
use crate::path_resolver;
use crate::GrpcAppState;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

pub struct GnmiService {
    state: GrpcAppState,
}

impl GnmiService {
    pub fn new(state: GrpcAppState) -> Self {
        Self { state }
    }

    pub fn into_gnmi_server(self) -> GnmiServer<Self> {
        GnmiServer::new(self)
    }
}

#[tonic::async_trait]
impl Gnmi for GnmiService {
    async fn capabilities(
        &self,
        _: Request<CapabilityRequest>,
    ) -> Result<Response<CapabilityResponse>, Status> {
        Ok(Response::new(CapabilityResponse {
            supported_models: vec!["netpilot-v1".into()],
            supported_encodings: vec!["JSON".into()],
            version: "0.1.0".into(),
        }))
    }

    async fn get(
        &self,
        req: Request<GetRequest>,
    ) -> Result<Response<GetResponse>, Status> {
        let paths = &req.get_ref().path;
        let state = self.state.clone();
        let values: Vec<PathValue> = paths
            .iter()
            .filter_map(|p| path_resolver::resolve(&state, p))
            .collect();
        Ok(Response::new(GetResponse { value: values }))
    }

    async fn set(
        &self,
        req: Request<SetRequest>,
    ) -> Result<Response<SetResponse>, Status> {
        let req = req.into_inner();

        let config_path = crate::proto::gnmi::v1::Path {
            elem: vec!["netpilot".into(), "config".into(), "candidate".into()],
        };

        for pv in &req.replace {
            if paths_equal(&pv.path, &config_path) {
                if let Some(ref value) = pv.value.as_ref() {
                    let config: netpilot_config::RoutePlaneConfig =
                        serde_json::from_slice(value).map_err(|e| {
                            Status::invalid_argument(format!("invalid config JSON: {e}"))
                        })?;
                    netpilot_config::validation::validate_config(&config).map_err(|e| {
                        Status::invalid_argument(format!("validation error: {e}"))
                    })?;
                    let mut store = self.state.config_store.write().map_err(|e| {
                        Status::internal(format!("lock error: {e}"))
                    })?;
                    store.replace_candidate(config).map_err(|e| {
                        Status::internal(format!("config store error: {e}"))
                    })?;
                    return Ok(Response::new(SetResponse {
                        value: vec![],
                        message: "candidate config updated".into(),
                    }));
                }
            }
        }

        if !req.update.is_empty() || !req.delete.is_empty() {
            return Err(Status::unimplemented(
                "gNMI update and delete not yet supported",
            ));
        }

        Ok(Response::new(SetResponse {
            value: vec![],
            message: "no changes applied".into(),
        }))
    }

    async fn subscribe(
        &self,
        req: Request<Streaming<SubscribeRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let mut stream = req.into_inner();
        let state = self.state.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        tokio::spawn(async move {
            while let Ok(Some(msg)) = stream.message().await {
                if let Some(subscribe) = msg.request {
                    if let Some(list) = subscribe.subscribe {
                        match SubscriptionList::Mode::try_from(list.mode) {
                            Ok(SubscriptionList::Mode::Once) => {
                                for sub in &list.subscription {
                                    if let Some(ref path) = sub.path {
                                        if let Some(pv) = path_resolver::resolve(&state, path) {
                                            let _ = tx
                                                .send(Ok(SubscribeResponse {
                                                    response: Some(
                                                        crate::proto::gnmi::v1::subscribe_response::Response::Update(pv),
                                                    ),
                                                }))
                                                .await;
                                        }
                                    }
                                }
                                let _ = tx
                                    .send(Ok(SubscribeResponse {
                                        response: Some(
                                            crate::proto::gnmi::v1::subscribe_response::Response::SyncResponse(true),
                                        ),
                                    }))
                                    .await;
                            }
                            Ok(SubscriptionList::Mode::Stream) => {
                                for sub in &list.subscription {
                                    if let Some(ref path) = sub.path {
                                        if let Some(pv) = path_resolver::resolve(&state, path) {
                                            let _ = tx
                                                .send(Ok(SubscribeResponse {
                                                    response: Some(
                                                        crate::proto::gnmi::v1::subscribe_response::Response::Update(pv),
                                                    ),
                                                }))
                                                .await;
                                        }
                                    }
                                }
                                let _ = tx
                                    .send(Ok(SubscribeResponse {
                                        response: Some(
                                            crate::proto::gnmi::v1::subscribe_response::Response::SyncResponse(true),
                                        ),
                                    }))
                                    .await;
                            }
                            _ => {
                                let _ = tx
                                    .send(Err(Status::unimplemented("POLL mode not supported")))
                                    .await;
                            }
                        }
                    }
                }
            }
        });

        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(out_stream) as Self::SubscribeStream))
    }
}

fn paths_equal(a: &crate::proto::gnmi::v1::Path, b: &crate::proto::gnmi::v1::Path) -> bool {
    a.elem == b.elem
}
```

- [ ] **Step 3: Write netpilot_svc.rs**

Write `crates/netpilot-grpc/src/netpilot_svc.rs`:
```rust
use crate::proto::netpilot::v1::{
    config_server::{Config, ConfigServer},
    health_server::{Health, HealthServer},
    CommitConfigRequest, CommitConfigResponse, GetRunningRequest, GetRunningResponse,
    HealthCheckRequest, HealthCheckResponse, PutCandidateRequest, PutCandidateResponse,
    RollbackRequest, RollbackResponse,
};
use crate::GrpcAppState;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub struct ConfigService {
    state: GrpcAppState,
}

impl ConfigService {
    pub fn new(state: GrpcAppState) -> Self {
        Self { state }
    }
    pub fn into_config_server(self) -> ConfigServer<Self> {
        ConfigServer::new(self)
    }
}

#[tonic::async_trait]
impl Config for ConfigService {
    async fn get_running(
        &self,
        _: Request<GetRunningRequest>,
    ) -> Result<Response<GetRunningResponse>, Status> {
        let store = self.state.config_store.read().map_err(|e| {
            Status::internal(format!("lock error: {e}"))
        })?;
        let config = store.running();
        let json =
            serde_json::to_vec(config).map_err(|e| Status::internal(format!("serialize: {e}")))?;
        let revision_id = store.revisions().last().map(|r| r.id as u64).unwrap_or(0);
        Ok(Response::new(GetRunningResponse {
            config_json: json,
            revision_id,
        }))
    }

    async fn put_candidate(
        &self,
        req: Request<PutCandidateRequest>,
    ) -> Result<Response<PutCandidateResponse>, Status> {
        let config: netpilot_config::RoutePlaneConfig =
            serde_json::from_slice(&req.get_ref().config_json)
                .map_err(|e| Status::invalid_argument(format!("invalid JSON: {e}")))?;
        netpilot_config::validation::validate_config(&config)
            .map_err(|e| Status::invalid_argument(format!("validation: {e}")))?;
        let mut store = self.state.config_store.write().map_err(|e| {
            Status::internal(format!("lock error: {e}"))
        })?;
        store.replace_candidate(config).map_err(|e| {
            Status::internal(format!("store error: {e}"))
        })?;
        Ok(Response::new(PutCandidateResponse {
            status: "candidate updated".into(),
        }))
    }

    async fn commit_config(
        &self,
        req: Request<CommitConfigRequest>,
    ) -> Result<Response<CommitConfigResponse>, Status> {
        let req = req.into_inner();
        let mut store = self.state.config_store.write().map_err(|e| {
            Status::internal(format!("lock error: {e}"))
        })?;
        let revision = store
            .commit(netpilot_config::CommitRequest {
                author: req.author,
                note: req.note,
            })
            .map_err(|e| Status::internal(format!("commit error: {e}")))?;
        Ok(Response::new(CommitConfigResponse {
            revision_id: revision.id as u64,
            message: format!("committed revision {}", revision.id),
        }))
    }

    async fn rollback(
        &self,
        req: Request<RollbackRequest>,
    ) -> Result<Response<RollbackResponse>, Status> {
        let req = req.into_inner();
        let mut store = self.state.config_store.write().map_err(|e| {
            Status::internal(format!("lock error: {e}"))
        })?;
        let revision = store
            .rollback(netpilot_config::RollbackRequest {
                revision_id: req.revision_id,
                author: req.author,
                note: req.note,
            })
            .map_err(|e| Status::internal(format!("rollback error: {e}")))?;
        Ok(Response::new(RollbackResponse {
            revision_id: revision.id as u64,
            message: format!("rolled back to revision {}", revision.id),
        }))
    }
}

pub struct HealthService {
    state: GrpcAppState,
}

impl HealthService {
    pub fn new(state: GrpcAppState) -> Self {
        Self { state }
    }
    pub fn into_health_server(self) -> HealthServer<Self> {
        HealthServer::new(self)
    }
}

#[tonic::async_trait]
impl Health for HealthService {
    async fn check(
        &self,
        _: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let store = self.state.config_store.read().map_err(|e| {
            Status::internal(format!("lock error: {e}"))
        })?;
        let revision = store.revisions().last().map(|r| r.id as u32).unwrap_or(0);
        Ok(Response::new(HealthCheckResponse {
            status: "SERVING".into(),
            uptime_secs: "0".into(),
            config_revision: revision,
        }))
    }

    type WatchStream = ReceiverStream<Result<HealthCheckResponse, Status>>;

    async fn watch(
        &self,
        _: Request<HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                let store = state.config_store.read().unwrap();
                let revision = store.revisions().last().map(|r| r.id).unwrap_or(0);
                drop(store);
                let _ = tx
                    .send(Ok(HealthCheckResponse {
                        status: "SERVING".into(),
                        uptime_secs: "0".into(),
                        config_revision: revision as u32,
                    }))
                    .await;
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
```

- [ ] **Step 4: Write path_resolver.rs**

Write `crates/netpilot-grpc/src/path_resolver.rs`:
```rust
use crate::proto::gnmi::v1::{Path, PathValue};
use crate::GrpcAppState;

pub fn resolve(state: &GrpcAppState, path: &Path) -> Option<PathValue> {
    let path_str = path.elem.join("/");
    let value: Option<Vec<u8>> = match path_str.as_str() {
        "netpilot/config/running" => {
            let store = state.config_store.try_read().ok()?;
            let config = store.running();
            serde_json::to_vec(config).ok()
        }
        "netpilot/state/health" => {
            Some(br#"{"status":"SERVING"}"#.to_vec())
        }
        "netpilot/state/protocols" => {
            let store = state.config_store.try_read().ok()?;
            let config = store.running();
            let names: Vec<&str> = config
                .protocols
                .iter()
                .map(|p| match p {
                    netpilot_config::ProtocolConfig::Static { name, .. } => name.as_str(),
                    netpilot_config::ProtocolConfig::Bgp { name, .. } => name.as_str(),
                    netpilot_config::ProtocolConfig::Ospf { name, .. } => name.as_str(),
                })
                .collect();
            serde_json::to_vec(&names).ok()
        }
        "netpilot/state/mpls/domains" => {
            let store = state.config_store.try_read().ok()?;
            let config = store.running();
            serde_json::to_vec(config.mpls_domains.as_deref().unwrap_or(&[])).ok()
        }
        "netpilot/state/sr/prefix-sids" => {
            let reg = state.sid_registry.try_read().ok()?;
            let sids = reg.list_prefix_sids();
            serde_json::to_vec(sids).ok()
        }
        _ => None,
    };
    value.map(|v| PathValue {
        path: Some(path.clone()),
        value: v,
    })
}
```

- [ ] **Step 5: Build check**

Run: `cargo build -p netpilot-grpc 2>&1`
Expected: PASS (or identify compilation errors, fix, then commit)

- [ ] **Step 6: Commit**

```bash
git add crates/netpilot-grpc/src/
git commit -m "feat: add gNMI, Config, Health gRPC service implementations (#315-#316)

GnmiService: Capabilities/Get/Set/Subscribe with ONCE and STREAM modes.
ConfigService: GetRunning/PutCandidate/CommitConfig/Rollback.
HealthService: Check + Watch streaming. Path resolver for 5 paths.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: gRPC Schema Fields

**Files:**
- Modify: `crates/netpilot-config/src/schema.rs`

- [ ] **Step 1: Add gRPC fields**

Add to RoutePlaneConfig:
```rust
    pub grpc_listen_addr: Option<String>,
    pub grpc_tls_cert_path: Option<String>,
    pub grpc_tls_key_path: Option<String>,
```

Add to Default impl:
```rust
            grpc_listen_addr: None,
            grpc_tls_cert_path: None,
            grpc_tls_key_path: None,
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p netpilot-config && git add crates/netpilot-config/src/schema.rs
git commit -m "feat: add gRPC listen address and TLS config fields to schema

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Integrate gRPC into netpilotd

**Files:**
- Modify: `crates/netpilotd/Cargo.toml`
- Modify: `crates/netpilotd/src/main.rs`

- [ ] **Step 1: Add dependency**

Add to `crates/netpilotd/Cargo.toml`:
```toml
netpilot-grpc = { path = "../netpilot-grpc" }
tonic = "0.12"
```

- [ ] **Step 2: Start gRPC server in main.rs**

Read current main.rs, add gRPC server startup after axum. The exact code depends on existing main.rs structure.

Commit message: `"feat: start gRPC server alongside axum in netpilotd"`

---

### Task 6: Integration Tests

**Files:**
- Create: `crates/netpilot-grpc/tests/gnmi.rs`

- [ ] **Step 1: Write proto + gRPC integration tests**

Write tests using tonic client to exercise Get/Set/Check RPCs against a test server.

- [ ] **Step 2: Run full test suite**

```bash
cargo test
```
Expected: All passing

---

### Final Verification

```bash
cargo fmt --check
cargo test
```
