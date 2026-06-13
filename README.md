# NetPilot — BIRD2-Compatible Routing Platform

A Rust-based routing daemon with full BIRD2 configuration compatibility, structured
config management, and modern management interfaces.

📖 **[完整介绍文档](docs/INTRODUCTION.md)** | 📋 **[配置参考](docs/CONFIGURATION.md)**

## Quick Start
```bash
git clone <repo>
cd NetPilot
cargo run -p netpilotd
```

Then open http://127.0.0.1:8080/ for the NOC dashboard.

## Architecture

NetPilot follows a microkernel architecture with protocol actors communicating
through a shared RIB (Routing Information Base) and event bus.

### Crate Layout (22 crates)

| Crate | Purpose |
|-------|---------|
| `netpilot-config` | Structured config schema, validation, diff, commit/rollback |
| `netpilot-protocol` | ProtocolActor trait, ProtocolSupervisor, event types |
| `netpilot-channel` | Protocol-to-RIB channel abstraction |
| `netpilot-rib` | Routing Information Base with best-route selection & ECMP |
| `netpilot-filter` | BIRD2-compatible filter language (nettype, AS-path, community) |
| `netpilot-birdconf` | BIRD2 config parser (bird.conf → structured config) |
| `netpilot-kernel` | Kernel routing table sync (netlink) |
| `netpilot-proto-bgp` | BGP protocol actor with TCP sessions & raw socket transport |
| `netpilot-proto-ospf` | OSPF protocol actor |
| `netpilot-proto-isis` | IS-IS protocol actor with SPF computation |
| `netpilot-proto-eigrp` | EIGRP protocol actor with DUAL algorithm |
| `netpilot-proto-ldp` | LDP protocol actor |
| `netpilot-proto-pim` | PIM protocol actor |
| `netpilot-proto-rip` | RIP protocol actor |
| `netpilot-grpc` | gRPC server (gNMI, gRIBI on :50051) |
| `netpilot-io` | Raw socket transport, CLI socket, WireGuard helpers |
| `netpilot-web` | React NOC dashboard (served via embedded dist) |
| `netpilotd` | Main daemon binary with REST API, CLI, gRPC |

### Interfaces
- **REST API** on `:8080` — config CRUD, health, SSE events
- **gRPC** on `:50051` — gNMI config management, gRIBI RIB injection
- **CLI socket** — interactive `netpilot-cli` interface
- **Web UI** at `http://127.0.0.1:8080/` — React NOC dashboard

## Features

- **100 BIRD2/FRR feature gap closure complete**
- **IS-IS, EIGRP, OSPF, BGP, LDP, PIM, RIP protocol actors**
- **MPLS** domain/label/SR support with static bindings
- **gNMI** management interface (Get, Set, Subscribe)
- **BIRD2 config parser** (bird.conf → structured config)
- **Real BGP TCP sessions** + raw socket transport
- **RIB** with best-route selection + ECMP
- **Segment Routing** (SR-MPLS prefix/adjacency SIDs, SRv6 locators/SIDs)
- **BGP-LS, BGPsec, BGP Flowspec**
- **SNMP, NETCONF/YANG, PBR, VRRP, SBFD, VNC**
- **Structured commit workflow** (candidate/running/rollback with confirmed-commit)
- **SSE real-time event stream** for route changes and protocol state transitions

## Current Status (2026-06-13)

**205 tests passing, 0 failures. 22 crates. 50+ commits.**

### What Works End-to-End
- REST API config CRUD (GET/PUT/POST candidate, commit, rollback, diff)
- gRPC/gNMI management interface (Capabilities, Get, Set, Subscribe)
- React Web UI dashboard with real-time SSE events
- BGP TCP sessions on port 179 (OPEN/KEEPALIVE/UPDATE encode/decode)
- RIB with best-route selection, ECMP, and recursive next-hop resolution
- Kernel FIB installation via netlink (Linux)
- Protocol actor framework (9 protocol actors)
- BIRD2 config parser (lexer + parser for static/BGP/OSPF blocks)
- MPLS label pool allocation/deallocation with range management
- Segment Routing prefix-SID registry with SRGB index resolution
- IS-IS Dijkstra SPF, EIGRP DUAL algorithm, OSPF SPF
- HMAC-based protocol authentication
- CI/CD pipeline (GitHub Actions, macOS + Linux)

### P0 Fixes Applied (12 fixes)
1. main.rs now spawns protocol actors from running config
2. Supervisor event_tx wired to all 9 protocol actors
3. BGP OPEN decode byte offset corrected (hold_time, bgp_identifier)
4. BGP persistent session loop with KEEPALIVE + reconnect
5. BGP UPDATE message decode support added
6. Kernel route messages now set real prefix/gateway/metric
7. RIB-to-Kernel FIB sync wired in event processor
8. IS-IS packet encode/decode functions added
9. IS-IS LoopbackTransport + runtime injectable transport
10. IS-IS hold timer decrement + adjacency expiry detection
11. EIGRP EigrpTransport trait + LoopbackTransport
12. OSPF real Dijkstra SPF replacing empty stub

## Configuration

Put configuration in the `configs/` directory or POST to `/api/config/candidate`:

```bash
# View running config
curl http://127.0.0.1:8080/api/config/running | jq

# Submit candidate config
curl -X PUT http://127.0.0.1:8080/api/config/candidate \
  -H "Content-Type: application/json" \
  -d '{"schema_version":1,"identity":{"router_id":"10.0.0.1"},"tables":[{"name":"master"}],"protocols":[]}'

# View diff
curl http://127.0.0.1:8080/api/config/diff | jq

# Commit
curl -X POST http://127.0.0.1:8080/api/config/commit \
  -H "Content-Type: application/json" \
  -d '{"author":"admin","note":"initial config"}'

# Commit with rollback timer (confirmed commit)
curl -X POST http://127.0.0.1:8080/api/config/commit-confirmed \
  -H "Content-Type: application/json" \
  -d '{"author":"admin","note":"test commit","timeout_secs":120}'
```

## SSE Real-Time Events

The `/api/events` endpoint streams ProtocolEvents as Server-Sent Events:

```javascript
const es = new EventSource('http://127.0.0.1:8080/api/events');
es.onmessage = (e) => {
  const event = JSON.parse(e.data);
  // event.type: "state_change", "route_announce", "route_withdraw", "error", "stats"
};
```

## Testing

```bash
cargo test                    # Unit & integration tests
cargo build                   # Full workspace build
./scripts/smoke-test.sh       # End-to-end smoke tests (if available)
```

## Development

```bash
# Run the daemon
cargo run -p netpilotd

# Build the web frontend (if modifying)
cd crates/netpilot-web
npm install
npm run build

# Run with custom config
cargo run -p netpilotd -- -c configs/example.json
```
