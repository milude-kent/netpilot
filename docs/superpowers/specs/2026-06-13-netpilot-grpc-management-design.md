# NetPilot M6 — gRPC & gNMI Management Interface Design

Date: 2026-06-13

## Goal

Implement full gRPC + gNMI management interface (#315-#316) with tonic, protobuf service definitions, path-based state access, and streaming telemetry. Covers gNMI Capabilities/Get/Set/Subscribe and NetPilot native Config/Health services.

## Scope

### In scope

| Feature | Reference | Description |
|---------|-----------|-------------|
| gRPC server | #316 | tonic-based server on configurable port, optional TLS |
| gNMI service | #315 | Capabilities, Get, Set, Subscribe (STREAM, ONCE, POLL) |
| NetPilot Config | #316 | GetRunning, PutCandidate, CommitConfig, Rollback RPCs |
| NetPilot Health | #316 | HealthCheck + Watch streaming RPCs |
| Path resolver | gNMI | Map gNMI paths to AppState data (config, health, protocols, MPLS, SR) |
| Proto definitions | proto | gnmi.proto (simplified openconfig/gnmi), netpilot.proto |
| Subscribe broadcast | gNMI | ON_CHANGE via tokio broadcast, SAMPLE via interval timer |
| gRPC TLS config | schema | grpc_listen_addr, grpc_tls_cert_path, grpc_tls_key_path seed fields |
| Integration tests | test | tonic client tests for Get, Set, Health Check |

### Out of scope

- SNMP agent (#312)
- YANG model definitions (#313)
- NETCONF/RESTCONF server (#314)
- Full OpenConfig model (interfaces, BGP, etc.)
- gNMI Delete support (returns UNIMPLEMENTED)
- gRPC authentication/authorization

## Architecture

```
netpilotd main.rs
  ├── axum server (:8080)      ← existing REST API
  └── tonic server (:50051)    ← new gRPC + gNMI
        └── shares AppState via Arc

New crate: netpilot-grpc/
  build.rs                     ← tonic-build
  proto/gnmi.proto              ← gNMI v1 spec
  proto/netpilot.proto          ← NetPilot Config/Health
  src/lib.rs                    ← gRPC server builder
  src/gnmi.rs                   ← gNMI service impl
  src/netpilot_svc.rs           ← Config + Health impl
  src/path_resolver.rs          ← path → AppState mapping
  tests/gnmi.rs                 ← integration tests
```

## Data Structures

### RoutePlaneConfig additions

```rust
pub struct RoutePlaneConfig {
    // ... existing ...
    pub grpc_listen_addr: Option<String>,        // default "0.0.0.0:50051"
    pub grpc_tls_cert_path: Option<String>,
    pub grpc_tls_key_path: Option<String>,
}
```

### Proto services

gNMI: Capabilities(SupportedModels, Encodings), Get(path→PathValue), Set(replace/update/delete), Subscribe(stream PathValue)

NetPilot: Config.GetRunning, Config.PutCandidate, Config.CommitConfig, Config.Rollback, Health.Check, Health.Watch(stream)

### Path resolver paths

`/netpilot/config/running`, `/netpilot/state/health`, `/netpilot/state/protocols`, `/netpilot/state/mpls/domains`, `/netpilot/state/sr/prefix-sids`

## Dependencies

tonic 0.12, prost 0.13, prost-types 0.13, tonic-build 0.12 (build). Shared AppState via Arc.

## Files Changed

| File | Change | Approx lines |
|------|--------|-------------|
| `crates/netpilot-grpc/` (7 files) | New crate | +475 |
| `crates/netpilot-config/src/schema.rs` | grpc fields | +15 |
| `crates/netpilotd/Cargo.toml` | add dep | +2 |
| `crates/netpilotd/src/main.rs` | tonic startup | +20 |
| `Cargo.toml` | workspace member | +3 |
| **Total** | | **~515** |

## Test Plan

- Proto round-trip: CapabilityRequest/Response, GetRequest/Response, SetRequest/Response, HealthCheckRequest/Response serialize/deserialize correctly
- Path resolver: each known path returns Some, unknown path returns None
- gRPC integration: tonic client → Get running config, Put candidate, Health Check
