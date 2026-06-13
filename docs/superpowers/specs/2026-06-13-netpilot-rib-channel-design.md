# NetPilot M4 — RIB + Channel Architecture Design

Date: 2026-06-13

## Goal

Build the Routing Information Base (RIB) and protocol-to-RIB channel abstraction as the data plane core of NetPilot. The RIB holds all routes, performs best-route selection, handles ECMP, and resolves recursive next hops. The channel engine provides import/export filter pipelines that connect protocol actors to the RIB.

## Scope

### In scope

| Component | Description |
|-----------|-------------|
| RouteTable (RIB) | Prefix-indexed routing table with LRU eviction and GC |
| BestRouteSelection | Preference/metric/gateway comparison with tie-breaking |
| ECMP | Equal-Cost Multi-Path with configurable max paths |
| Recursive next-hop resolution | Chase next-hops through the RIB until direct |
| RouteEntry | Full route attributes: prefix, gateway, metric, protocol, source, as_path, communities, mpls_label, igp_metric |
| ChannelEngine | Import filter pipeline: reject/default-accept → filter → table insert |
| ChannelLimits | Import/export limits with action (warn/block/restart/disable) |
| ChannelStats | Per-channel counters: imported, exported, filtered, active |
| Kernel sync integration | RIB change notifications trigger FIB install/delete |

### Out of scope

- Route table synchronization between peers (BGP RIB-OUT)
- Inter-table route leaking
- Multi-VRF route isolation
- EVPN MAC/IP advertisement tables

## Architecture

```
netpilot-rib/
  src/lib.rs              ← re-exports, RIB handle
  src/table.rs            ← RouteTable<T>: HashMap+LRU, insert/remove/lookup
  src/selection.rs        ← best_route_select(), ECMP path selection
  src/nexthop.rs          ← recursive next-hop resolution
  src/entry.rs            ← RouteEntry struct + RouteAttributes
  src/gc.rs               ← Garbage collection: threshold-based sweep

netpilot-channel/
  src/lib.rs              ← re-exports
  src/engine.rs           ← ChannelEngine: import/export pipelines
  src/limits.rs           ← ChannelLimits, LimitAction, limit enforcement
  src/stats.rs            ← ChannelStats: counters + rate calculations
```

Dependencies: `netpilot-rib` depends on `netpilot-config`. `netpilot-channel` depends on `netpilot-rib` + `netpilot-filter`.

## Key Types

**RouteTable<T>:** Generic over table type. `insert(prefix, route)`, `remove(prefix)`, `lookup_prefix(prefix)`, `lookup_exact(prefix)`, `dump()`, `gc()`

**BestRouteSelection:** `select(routes: &[RouteEntry]) -> Option<&RouteEntry>` — preference desc, metric asc, router_id tie-break. For ECMP, returns multiple equal-cost paths.

**Recursive resolution:** Given a gateway `10.0.0.2`, look up `10.0.0.2/32` in the RIB. If resolved gateway is also non-direct, recurse until a directly-connected gateway is found or the resolution limit (16) is exceeded.

**ChannelEngine:** `import(routes, limits, filter_fn)` — validates limit, applies filter, inserts. `export(prefix, filter_fn)` — looks up route, applies export filter, returns result.

**ChannelLimits:** `import_limit`, `import_limit_action`, `receive_limit`, `receive_limit_action`, `export_limit`, `export_limit_action`

## Data Flow

```
ProtocolActor::RouteAnnounce
  → Supervisor::broadcast(ProtocolEvent::RouteAnnounce { routes })
    → ChannelEngine::import()
      → Filter::evaluate() [accept/reject]
        → RouteTable::insert(prefix, route)
          → BestRouteSelection::select() [triggers on table change]
            → FIB sync [install/delete in kernel]
```

## Files Changed

| File | Approx lines |
|------|-------------|
| `crates/netpilot-rib/Cargo.toml` | +15 |
| `crates/netpilot-rib/src/lib.rs` | +30 |
| `crates/netpilot-rib/src/table.rs` | +150 |
| `crates/netpilot-rib/src/selection.rs` | +100 |
| `crates/netpilot-rib/src/nexthop.rs` | +80 |
| `crates/netpilot-rib/src/entry.rs` | +60 |
| `crates/netpilot-rib/src/gc.rs` | +50 |
| `crates/netpilot-channel/Cargo.toml` | +15 |
| `crates/netpilot-channel/src/lib.rs` | +30 |
| `crates/netpilot-channel/src/engine.rs` | +100 |
| `crates/netpilot-channel/src/limits.rs` | +60 |
| `crates/netpilot-channel/src/stats.rs` | +40 |
| Tests (both crates) | +180 |
| **Total** | **~910** |

## Design Decisions

1. **RIB is single-writer, multi-reader**: Table mutations happen only through ChannelEngine. Lookups (for route resolution, CLI show commands) use shared read access via RwLock.

2. **ECMP is route-level, not per-packet**: The RIB stores multiple equal-cost paths. Packet-level load balancing is a forwarding plane concern.

3. **Recursive resolution depth-limited**: Maximum 16 levels of recursion to prevent infinite loops from misconfigured static routes.

4. **GC is threshold-based**: When route count exceeds `gc_threshold`, a background sweep removes aged-out routes (stale time > `gc_period_secs`). Enabled per-table.

5. **Channel engine is protocol-agnostic**: Import/export pipelines are generic over filter expressions. Any ProtocolActor can connect a channel to any table.

6. **Kernel FIB sync is edge-triggered**: RIB table change events drive FIB operations. No polling, no periodic reconciliation (reconciliation added later if needed).

For the canonical implementation, see `crates/netpilot-rib/` and `crates/netpilot-channel/`.
