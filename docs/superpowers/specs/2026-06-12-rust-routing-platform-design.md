# RiftCore Routing Platform Design

Date: 2026-06-12

## Goal

Build **RiftCore**, a Rust-based routing platform similar in scope to BIRD2, with a Web management plane, Linux server/cloud host as the first target, and support for a candidate/running configuration workflow with commit, confirmed commit, rollback, audit, and future controller/agent split deployment.

The selected architecture is **Rust microkernel + protocol actors**. The system should run as a single all-in-one deployment first, while preserving boundaries that allow a future controller to manage multiple routing agents.

## 1. System Boundary

The main process is `routerd`. In the first stage, it runs the routing core, API, Web UI, CLI entry points, protocol actors, policy engine, and Linux kernel adapter in one deployment.

Later, the same architecture can split into:

- `controller`: Web UI, external API, configuration, audit, multi-node management.
- `agent`: routing protocols, RIB, policy execution, Linux netlink/FIB synchronization.

Layering:

```text
Web UI / CLI / API
        |
Candidate / Running Config + Commit / Rollback
        |
Policy VM + RIB Core + Protocol Supervisor
        |
BGP / OSPF / RIP / Babel / Static / Direct / Kernel / BFD / RPKI / MRT
        |
Linux Netlink / Kernel Route Table / Interfaces / VRF
```

Rules:

- Protocol modules do not write Linux routes directly. They submit route candidates to the RIB core.
- The RIB core performs table isolation, best-path selection, ECMP decisions, next-hop resolution, and route lifecycle tracking.
- The kernel adapter synchronizes selected route diffs to Linux FIB through netlink.
- Web/API/CLI never mutate running protocol state directly. They edit candidate config, validate, diff, and commit.
- Protocol modules are Rust crates linked into the daemon first. Dynamic plugin loading is not part of the first design because Rust ABI stability and security boundaries would add avoidable complexity.

## 2. Protocol Coverage

The protocol surface should be broad enough to map to BIRD2-style use cases. Implementation order can still prioritize the smallest route-to-kernel loop first.

### Basic Route Sources

- `direct`: discovers local interface addresses and directly connected routes.
- `static`: supports static, blackhole, unreachable, and recursive next-hop routes.
- `kernel`: imports and exports Linux kernel routes.
- `device`: watches interface, address, link-state, MTU, and VRF changes.

### Core Dynamic Protocols

- `bgp`: eBGP/iBGP, IPv4/IPv6 unicast, route refresh, graceful restart, communities, large communities, local-pref, MED, AS path, next-hop self, and multipath.
- `ospf`: OSPFv2/OSPFv3, areas, interface cost, stub/NSSA, LSDB, SPF, and ECMP.
- `rip`: RIPv2/RIPng for compatibility.
- `babel`: preserved as a BIRD2-aligned module boundary, with lower implementation priority than BGP and OSPF.

### Auxiliary Protocols and Tools

- `bfd`: fast failure detection. BGP, OSPF, and Static subscribe to BFD session state through the event bus.
- `rpki`: RTR client, ROA cache, and RPKI validation state for BGP import policy.
- `mrt`: BGP and RIB dumps for troubleshooting, archival, and external analysis.
- `pipe`: route copying between tables, VRFs, or tenants with policy isolation.

All protocols use a shared actor interface:

```text
Protocol Actor
  -> receives config snapshots
  -> emits route updates and protocol events
  -> receives dependency events such as BFD down or interface down
  -> exposes status, counters, and recent errors
```

Protocol modules do not call each other directly. Shared dependencies flow through the core event bus.

## 3. Configuration System

Configuration uses a two-layer model:

```text
BIRD-like DSL / Web Forms / CLI
            |
        Config API
            |
Structured Config AST / Schema
            |
Validate -> Diff -> Commit -> Running Config
```

### Source of Truth

- `running config` is the active configuration.
- `candidate config` is the editable pending configuration.
- The durable internal representation is structured and versioned. JSON/YAML plus a schema version is sufficient for the file-backed first stage.
- A SQLite revision log records commit history, metadata, and snapshots.
- The BIRD-like DSL is an input layer. It is parsed into structured configuration and is not the only source of truth.

### Commit Flow

```text
edit candidate
  -> parse / normalize
  -> schema validate
  -> semantic validate
  -> dependency check
  -> dry-run diff
  -> commit transaction
  -> protocol actors receive scoped config update
  -> health check
  -> mark commit success or rollback
```

Validation covers:

- Syntax and schema checks.
- Protocol completeness, such as BGP neighbor ASN, remote address, and AFI/SAFI.
- Policy references.
- Table, VRF, and interface references.
- Inter-protocol dependencies, such as BGP neighbors referencing an existing BFD profile.
- Dangerous diffs, including deleting many BGP neighbors, clearing export policy, or changing router-id.

### Rollback

- Every commit creates a revision id.
- Each revision stores a full snapshot, structured diff, commit source, user, time, and note.
- Rollback to a specified revision is supported.
- Confirmed commit is supported, such as `commit confirmed 120`. If the commit is not confirmed within 120 seconds, the system automatically rolls back.
- The Web UI requires extra confirmation for high-risk changes.

### Hot Reload

- Small changes use scoped actor reloads. For example, changing one BGP neighbor policy should only affect that neighbor.
- Changes that cannot safely hot reload restart the affected protocol instance. For example, changing an OSPF router-id should restart the OSPF instance.
- Kernel route synchronization is always driven by RIB diffs, not by blindly applying configuration over the kernel table.

## 4. RIB, Policy VM, and Best Path

Route flow:

```text
Protocol RIB-In
    -> Import Policy
        -> Master RIB / per-table RIB
            -> Best Path Selection
                -> Export Policy
                    -> Protocol RIB-Out / Kernel FIB
```

### RIB Model

- Each protocol maintains its own protocol RIB-In.
- Routes accepted by import policy enter a core table RIB.
- Multiple routing tables are supported for Linux tables, VRFs, tenants, or pipe isolation.
- A route record includes prefix, AFI/SAFI, next-hop, metric, preference, protocol source, protocol attributes, and lifecycle state.
- Lifecycle states include active, hidden, filtered, stale, and withdrawn.

### Best Path Selection

- Selection is isolated per table.
- Cross-protocol route preference is configurable, for example Static over BGP over OSPF.
- Protocol-specific best-path logic belongs to the protocol module. BGP best path and OSPF SPF are not generic RIB rules.
- ECMP is supported when next-hops are resolvable and policy allows multipath.
- The RIB core emits selected route diffs to export policy and the kernel adapter.

### Policy VM

The policy path compiles DSL into a constrained executable form:

```text
DSL source -> parser -> AST -> bytecode / IR -> Policy VM
```

Policy supports:

- prefix-set, community-set, and as-path-set.
- match by prefix, protocol, interface, table, RPKI state, and BGP attributes.
- set local-pref, MED, community, next-hop, preference, and tag.
- accept, reject, continue, and call chain.
- policy dry-run for a sample route, including decision trace and matched rules.

Policy VM constraints:

- No file I/O, network access, or system calls.
- Execution step limits prevent infinite loops.
- Policy changes must compile before commit.
- Web UI provides policy impact previews showing affected neighbors, tables, and exports.

## 5. Web Management Plane and API

The Web UI is an operations workbench. It should be dense, status-oriented, and suitable for repeated network operations.

### Dashboard

- router-id, ASN, uptime, and version.
- Protocol status overview: BGP established/down, OSPF neighbors, BFD sessions, and RPKI cache state.
- RIB counts: active, hidden, filtered, and total routes.
- Kernel sync status and recent errors.

### Protocols

- BGP neighbor list, session state, prefixes in/out, last error, and flap count.
- OSPF area, interface, neighbor, and LSDB summary.
- Static, Direct, Kernel, RIP, Babel, BFD, RPKI, and MRT module status.
- Protocol instance detail pages with counters, logs, and recent events.

### Routes

- Search by table, VRF, protocol, and prefix.
- Show best, hidden, and filtered routes.
- Route detail includes attributes, policy decision, next-hop resolution, and kernel install state.

### Policy

- Edit prefix-sets, community-sets, and as-path-sets.
- Support both DSL editor and structured form entry.
- Provide policy dry-run and impact preview.
- Show hit counters and recent rejected route samples.

### Configuration

- Candidate/running diff.
- Commit, confirmed commit, and rollback.
- Revision history with user, note, time, source, and structured diff.
- Dangerous diff warning and confirmation flow.

### API

- Internal service boundaries use typed Rust traits first.
- Remote agent/controller communication is reserved for `tonic` gRPC.
- External API is REST/OpenAPI for Web UI, automation scripts, and third-party systems.
- Web UI never touches actor internals directly. It calls the API.
- In split mode, agents expose gRPC while the controller manages node inventory, config distribution, and audit.

### Permissions

- First stage supports admin, operator, and viewer roles.
- Commit, rollback, session reset, protocol restart, and config export enter the audit log.
- Initial authentication can be local users or reverse-proxy integration.
- Later authentication can add OIDC or LDAP.

## 6. Rust Technical Stack and Testing

### Backend

- Async runtime: `tokio`.
- Web/API: `axum` plus REST/OpenAPI.
- Future remote RPC: `tonic` gRPC.
- Actor/event model: `tokio` channels plus an explicit protocol supervisor.
- Linux netlink: a dedicated `kernel-adapter` crate wrapping route, address, link, and VRF operations.
- Config schema: `serde`.
- DSL parser: a parser crate that outputs AST and structured config.
- Storage: file snapshots plus SQLite revision log first; PostgreSQL can be added later for controller mode.

### Frontend

Use React/Vite/TypeScript or SvelteKit. The UI consumes the REST API and treats the backend as the source of truth. The first screen is the operations dashboard, not a landing page.

### Process Model

First stage:

```text
routerd
  +- config manager
  +- API server
  +- web static server
  +- RIB core
  +- policy engine
  +- protocol supervisor
  +- protocol actors
  +- kernel adapter
```

Future split:

```text
controller
  +- Web / API / config / audit

agent
  +- protocols / RIB / policy / kernel sync
```

### Testing

- Unit tests for RIB selection, policy VM, configuration diff, and protocol state machines.
- Protocol tests for BGP OPEN/UPDATE/KEEPALIVE, OSPF LSA/SPF, and BFD state transitions.
- Integration tests using Linux network namespaces to simulate router topologies.
- Golden tests for BIRD-like DSL input to AST/schema output.
- Rollback tests for confirmed commit timeout and recovery.
- Kernel tests comparing netlink route add/delete behavior with observed `ip route` state.
- Web tests for config submission, diff review, rollback, and route search.

### Recommended Build Order

1. `routerd` core framework, config revision storage, and API skeleton.
2. Static, Direct, and Kernel modules to prove the RIB-to-FIB loop.
3. BGP protocol actor.
4. Policy VM and DSL parser.
5. OSPF, RIP, Babel, BFD, RPKI, MRT, and Pipe modules.

## Open Decisions

- Whether the first frontend should use React/Vite/TypeScript or SvelteKit.
- Whether the first durable config format should be YAML or JSON.
- Exact role permission matrix for admin/operator/viewer.
- Exact BIRD-like DSL compatibility target.

## Non-Goals for the First Implementation Plan

- Dynamic binary plugin loading.
- Full multi-node controller orchestration.
- Full BIRD2 configuration compatibility.
- Hardware router interface management beyond Linux server/cloud host needs.
