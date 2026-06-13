# NetPilot M6 — Protocol Actor Framework & Netlink Foundation Design

Date: 2026-06-13

## Goal

Build the protocol actor runtime framework (`netpilot-protocol`) and Linux netlink kernel interface (`netpilot-kernel`) as the foundation for all protocol implementations. The actor framework provides the trait, message types, handle, and supervisor that every protocol (IS-IS, EIGRP, BGP, OSPF) will use. The kernel crate provides safe async netlink route operations and interface event streaming.

## Scope

### In scope

| Component | Description |
|-----------|-------------|
| ProtocolActor trait | Async trait: `run(name, config, rx)` → spawnable in a tokio task |
| ProtocolMsg | Commands from daemon to protocol: Reload, Enable, Disable, Restart, Shutdown, StatusQuery |
| ProtocolEvent | Events from protocol to daemon: RouteAnnounce, RouteWithdraw, StateChange, Error, Stats |
| ProtocolHandle | Cloneable sender channel for daemon→protocol communication |
| ProtocolSupervisor | Spawns actors, tracks handles, broadcasts events to subscribers |
| KernelRouteClient | Netlink route operations: add, delete, dump, apply_diff |
| InterfaceWatcher | Async stream of link up/down and address add/remove events |
| Daemon integration | Supervisor + optional kernel client in AppState |

### Out of scope

- IS-IS protocol full implementation (next sub-project)
- EIGRP protocol full implementation (next sub-project)
- RIB route table core (next sub-project)
- BGP/OSPF actor rewrites

## Architecture

```
netpilot-protocol/
  src/lib.rs            ← trait + re-exports
  src/actor.rs          ← ProtocolActor trait, ProtocolMsg
  src/event.rs          ← ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes
  src/handle.rs         ← ProtocolHandle (sending side)
  src/supervisor.rs     ← ProtocolSupervisor (spawn, subscribe, shutdown)

netpilot-kernel/
  src/lib.rs            ← re-exports
  src/route.rs          ← KernelRoute, KernelRouteClient, RouteProtocol, KernelError
  src/interface.rs      ← InterfaceWatcher, InterfaceInfo, InterfaceEvent, AddressScope
```

Dependencies: `netpilot-protocol` depends only on `tokio` + `async-trait` + `netpilot-config`. `netpilot-kernel` depends on `rtnetlink` + `netlink-packet-route` + `futures`.

## Key Types

**ProtocolActor:** `async fn run(&mut self, name: String, config: ProtocolConfig, rx: mpsc::Receiver<ProtocolMsg>) -> Result<(), ProtocolError>`

**ProtocolMsg:** Reload{config, scope}, Enable, Disable, Restart, Shutdown, GracefulRestart, StatusQuery{reply: oneshot::Sender<ProtocolStatus>}

**ProtocolEvent:** RouteAnnounce, RouteWithdraw, StateChange, Error, Stats

**ProtocolHandle:** Cloneable, wraps `mpsc::Sender<ProtocolMsg>`, provides typed async methods with error handling

**ProtocolSupervisor:** `spawn<A: ProtocolActor>(&mut self, name, config, actor) -> ProtocolHandle`, `subscribe() -> broadcast::Receiver<ProtocolEvent>`, `shutdown_all()`

**KernelRouteClient:** `add(route)`, `delete(route)`, `dump(table_id)`, `apply_diff(old, new)`

**InterfaceWatcher:** `watch() -> impl Stream<Item = InterfaceEvent>`

## Files Changed

| File | Change | Approx lines |
|------|--------|-------------|
| `crates/netpilot-protocol/Cargo.toml` | New | +15 |
| `crates/netpilot-protocol/src/lib.rs` | New | +30 |
| `crates/netpilot-protocol/src/actor.rs` | New | +60 |
| `crates/netpilot-protocol/src/event.rs` | New | +80 |
| `crates/netpilot-protocol/src/handle.rs` | New | +70 |
| `crates/netpilot-protocol/src/supervisor.rs` | New | +100 |
| `crates/netpilot-kernel/Cargo.toml` | New | +15 |
| `crates/netpilot-kernel/src/lib.rs` | New | +20 |
| `crates/netpilot-kernel/src/route.rs` | New | +150 |
| `crates/netpilot-kernel/src/interface.rs` | New | +100 |
| `crates/netpilot-kernel/src/error.rs` | New | +30 |
| `crates/netpilotd/src/state.rs` | Modify | +20 |
| `Cargo.toml` | Modify workspace | +3 |
| Tests | Both crates | +200 |
| **Total** | | **~893** |

## Test Plan

- ProtocolHandle sends/receives all message types correctly
- ProtocolSupervisor spawns mock actor, returns handle, receives events
- Supervisor shutdown_all terminates all actors
- KernelRouteClient add/delete/dump (Linux only, or stub on macOS)
- InterfaceWatcher produces events (Linux only, or empty stream on macOS)
- ProtocolStatus query via oneshot channel round-trips
