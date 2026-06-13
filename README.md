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

**276 tests passing, 0 failures, 0 ignored. 22 crates. Production-hardened P0 complete.**

### What Works End-to-End
- REST API config CRUD (GET/PUT/POST candidate, commit, rollback, diff)
- gRPC/gNMI management interface (Capabilities, Get, Set, Subscribe with real event streaming)
- React Web UI dashboard with real-time SSE events
- BGP TCP sessions on port 179 with **RFC 4271-compliant OPEN/KEEPALIVE/UPDATE/NOTIFICATION encode/decode** (full capabilities, extended-length attributes, AS_PATH, IPv6 NLRI shape)
- RIB with best-route selection, ECMP, and recursive next-hop resolution
- **Kernel FIB installation via netlink (Linux)** — real `Oif` resolution from interface name to ifindex, IPv4 + IPv6, ECMP, MPLS-label stack support
- Protocol actor framework with 9 protocol actors and supervisor that recovers from lagged channels + panics
- BIRD2 config parser (lexer + parser for static/BGP/OSPF blocks)
- MPLS label pool allocation/deallocation with range management
- Segment Routing prefix-SID registry with SRGB index resolution
- IS-IS Dijkstra SPF, EIGRP DUAL algorithm, OSPF SPF
- **Authentication & authorization** — REST + gRPC bearer token (HMAC-SHA256 over expiry), optional mTLS for gRPC, allowlisted `/health` + `/metrics`, SSE auth via `?token=` query param
- **Real HMAC-MD5/SHA1/SHA256/SHA384/SHA512 + Blake2{s,b} MAC** for protocol authentication (KeyedMd5/Sha1 are now proper HMAC, not vulnerable H(key‖data)); constant-time comparison via `subtle::ConstantTimeEq`; key buffers zeroized on drop
- **Confirmed-commit auto-rollback** — `CommitScheduler` spawns a tokio task that auto-undoes a pending commit if `/api/config/confirm` is not called within the timeout window
- **RPKI RTR client** (RFC 6810/8210) — ingests ROAs + ASPAs over TCP from a configured cache, supports Reset and Serial queries, periodic refresh
- **Prometheus metrics endpoint** at `/metrics` (text 0.0.4 exposition format) with counters for events, FIB ops, supervisor restarts, BGP message I/O
- **Structured JSON logging** via `tracing-subscriber` (controlled by `RUST_LOG`)
- **Linux `drop_privileges`** via `capctl` — retains `CAP_NET_ADMIN`/`CAP_NET_RAW`/`CAP_NET_BIND_SERVICE`/`CAP_DAC_OVERRIDE`, drops `CAP_SYS_ADMIN`/`CAP_SYS_MODULE`/`CAP_SETUID`/etc.
- **CI/CD pipeline** (GitHub Actions) — fmt + clippy (hard gate, no `|| true`) + nextest + cargo-deny + llvm-cov + Linux smoke test + Windows build + release artifacts on tag

### Production-Hardening P0 Highlights
(See `docs/superpowers/plans/2026-...-netpilot-prod-hardening-p0.md` and `/Users/youchen/.claude/plans/iridescent-splashing-fountain.md` for the full plan.)

- **B1 auth**: real HMAC + Blake2 + ConstantTimeEq + Zeroizing — 28 tests including RFC 4231/2202/7693 vectors
- **B2 drop_privileges**: real capctl-based capability drop + uid/gid setresuid; integration test re-execs and asserts `/proc/self/status`
- **B3 BGP UPDATE**: full RFC 4271 encode/decode with attribute round-trip tests + in-process TCP loopback test
- **B4 Kernel FIB**: real IPv4+IPv6+Oif+ECMP+metric; netns-isolated integration tests
- **C1 auth middleware**: REST + gRPC Bearer token + optional mTLS; 9 auth-flow tests
- **C2 confirmed-commit scheduler**: auto-rollback via tokio task; 3 timing tests
- **C3 RPKI RTR client**: hand-rolled RFC 6810/8210 client + mock RTR integration test
- **C4 supervisor**: explicit Lagged/Closed handling; `spawn_supervised` panic recovery; 9 resilience tests
- **C5 SSE/Subscribe/metrics**: real event-driven gNMI Subscribe, Prometheus `/metrics`, SSE Lagged warning
- **C6 tracing**: JSON output via `tracing-subscriber`; all `eprintln!` migrated
- **D1-D7 tests**: 4 in-process integration tests, 2 new commit-flow tests, 3 criterion benches
- **E CI/CD**: clippy `-D warnings` hard gate, Linux smoke job, cargo-deny supply-chain, Codecov coverage, GHA release artifacts

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
