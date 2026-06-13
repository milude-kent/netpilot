# NetPilot — BIRD2-Compatible Routing Platform

A Rust-based routing daemon with full BIRD2 configuration compatibility, structured
config management, and modern management interfaces.

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

### Crate Layout (15 crates)

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
