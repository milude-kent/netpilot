# NetPilot BIRD2 Full Replacement Design

Date: 2026-06-12

## Goal

Build **NetPilot**, a Rust-based routing daemon and Web management platform whose long-term target is to be a complete operational replacement for **BIRD 2.x**, using **BIRD 2.19.1** as the first explicit compatibility baseline.

The selected product scope is option **C: complete replacement**:

- Functional equivalence with BIRD2 routing behavior.
- Compatibility with existing BIRD2 configuration files, including `bird.conf` style syntax.
- A first-class Web UI, REST API, and future controller/agent mode for safer operations.
- A native structured configuration model that can represent everything accepted from BIRD2 syntax.

NetPilot should not be a thin wrapper around BIRD. It should be an independent Rust implementation with a BIRD2-compatible front door and a Web/API management plane.

Official reference baseline:

- BIRD 2.19.1 User's Guide: <https://bird.nic.cz/doc/bird-2.19.1.html>
- BIRD latest documentation index: <https://bird.nic.cz/doc/latest/>

## Scope Statement

NetPilot has two related but separate goals:

1. **Compatibility goal:** existing BIRD2 users can migrate configurations, route policy, protocol behavior, status inspection, and operational workflows with minimal changes.
2. **Operations goal:** users can manage the same routing system through Web/API workflows with candidate/running config, diff, commit, confirmed commit, rollback, audit, status dashboards, route search, and policy simulation.

The compatibility target is broad and must be delivered in phases. The first implementation plans should build the core architecture and compatibility matrix before attempting every protocol at once.

## Current Project Baseline

Milestone 1 already provides:

- `netpilot-config`: structured config schema, validation, diff, candidate/running state, commit, rollback, and revision tracking.
- `netpilotd`: all-in-one daemon foundation with REST endpoints for health, running config, candidate config, diff, commit, and rollback.
- Initial structured protocol config for Static and BGP.

This design replaces the earlier "similar to BIRD2" language with "BIRD2 full replacement" and expands the target surface accordingly.

## Compatibility Definition

NetPilot compatibility is measured in five levels.

### Level 0: Inventory

The feature is listed in the compatibility matrix with a known status, owner crate, dependencies, and tests required.

### Level 1: Parse Compatible

NetPilot can parse the relevant BIRD2 syntax, preserve enough AST information for useful diagnostics, and convert it into a normalized internal model.

### Level 2: Configure Compatible

NetPilot can validate and commit the feature through both native structured config and BIRD2-compatible config input.

### Level 3: Runtime Compatible

NetPilot implements the runtime behavior: protocol state machine, route exchange, attributes, filters, kernel sync, counters, and reload behavior.

### Level 4: Operationally Compatible

NetPilot exposes equivalent or better operational controls through CLI, control socket/API, logs, status, route inspection, graceful restart, troubleshooting data, and Web UI.

## BIRD2 Feature Matrix

The BIRD 2.19.1 protocol list is the authoritative initial matrix. NetPilot must track each item explicitly.

| BIRD2 area | NetPilot target | Notes |
| --- | --- | --- |
| Aggregator | Full protocol support | Aggregate routes from selected sources with policy controls and export behavior. |
| Babel | Full protocol support | Include IPv4, IPv6, source-specific IPv6 routes where applicable, interface behavior, metrics, and route exchange. |
| BFD | Full service support | Service protocol without route channels; used by BGP, OSPF, Static, and other protocols for fast failure detection. |
| BGP | Full protocol support | BGPv4, IPv4/IPv6, multiprotocol extensions, route refresh, graceful restart, communities, large communities, extended communities, route server use, add-path where selected, confederations where compatible, RPKI policy hooks, BMP/MRT integration. |
| BMP | Full monitoring support | Export BGP monitoring data to BMP collectors with configurable peers, route monitoring scope, and status visibility. |
| Bridge | Full route type support | Ethernet forwarding entries and bridge/VXLAN related behavior where supported by the Linux target. |
| Device | Full service support | Interface, address, link-state, MTU, and VRF tracking; no route channels. |
| Direct | Full protocol support | Directly connected route discovery and import through channels. |
| EVPN | Full protocol support | EVPN route types, MAC/IP advertisements, IMET, route distinguisher, VNI/MPLS labels, and BGP integration. |
| Kernel | Full protocol support | Import/export between NetPilot tables and kernel FIBs through netlink on Linux; later BSD adapters may be added. |
| L3VPN | Full protocol support | VPNv4/VPNv6 route handling with route distinguisher, route targets, MPLS labels, and BGP integration. |
| MRT | Full dump support | MRT dumps for BGP and RIB troubleshooting and archival. |
| OSPF | Full protocol support | OSPFv2 and OSPFv3, areas, LSDB, SPF, ECMP, interface options, authentication, graceful handling where applicable. |
| Perf | Full diagnostic support | Performance counters, internal timing, route processing statistics, and operator-visible profiling data. |
| Pipe | Full protocol support | Route transfer between tables with import/export filters. |
| RAdv | Full protocol support | IPv6 Router Advertisement generation, prefix options, route info, and neighbor/discovery integration where applicable. |
| RIP | Full protocol support | RIPv2 and RIPng with authentication, timers, metrics, and import/export filters. |
| RPKI | Full protocol support | RTR client, ROA/ASPA data, validation state, policy hooks, cache status, and failure modes. |
| Static | Full protocol support | Static, blackhole, unreachable, prohibit, recursive next hop, multipath, BFD integration, route attributes. |

The matrix also includes non-protocol BIRD2 surfaces:

| BIRD2 surface | NetPilot target |
| --- | --- |
| Routing tables | Multiple independent tables by nettype, default `master4` and `master6`, explicit additional tables. |
| Network types | IPv4, IPv6, IPv6 SADR, VPN4, VPN6, ROA4, ROA6, ASPA, Flow4, Flow6, ETH, MPLS, EVPN, Neighbor. |
| Channels | Per-protocol table bindings with import/export filters and channel-level options. |
| Filters | BIRD2-compatible filtering language plus native structured policy model. |
| Route attributes | Common attributes and protocol-specific attributes with typed storage and policy access. |
| Reconfiguration | Hot reload, soft reconfiguration where semantically valid, scoped protocol restarts where required. |
| Remote control | `birdc`-like CLI/control socket semantics plus REST/API equivalents. |
| Graceful restart | Kernel and BGP baseline first, then additional protocol-specific support where BIRD2 supports or expects it. |
| Privileges | Linux capability-aware process model for routing sockets, raw sockets, netlink, and safe privilege dropping. |
| Logging | File/syslog/stderr-compatible targets plus structured logs for Web/API audit. |

## Architecture

The selected architecture remains **Rust microkernel + protocol actors**, but the compatibility target adds explicit parser, filter, route-type, channel, and control-socket subsystems.

```text
Web UI / CLI / birdc-compatible control / REST API
        |
Candidate / Running Config + Revision Store + Audit
        |
BIRD2 Config Parser -> AST -> Normalizer -> Structured Config
        |
Policy Compiler + Filter VM
        |
RIB Core + Nettype Tables + Channel Engine + Protocol Supervisor
        |
Protocol Actors and Service Actors
        |
Linux Netlink / Kernel FIB / Interfaces / VRF / Raw Sockets
```

Core rules:

- Protocol modules never write kernel routes directly. They emit route candidates and protocol events.
- The RIB core owns table isolation, route lifecycle, best-route selection, next-hop resolution, route replacement, hidden/suboptimal routes, and route diffs.
- The channel engine owns import/export filter application between protocol instances and tables.
- The kernel adapter applies selected diffs to Linux FIBs through netlink.
- The config manager owns parse, validate, diff, commit, confirmed commit, rollback, and revision history.
- Web/API/CLI/control socket never mutate protocol internals directly. They use command APIs exposed by the daemon.
- Dynamic binary plugin loading is not in the first target. Protocol modules are Rust crates linked into `netpilotd`.

## Crate Layout

The workspace should evolve toward these crates:

| Crate | Responsibility |
| --- | --- |
| `netpilot-config` | Structured config schema, revision store, validation, diff, candidate/running workflow. |
| `netpilot-birdconf` | BIRD2-compatible lexer, parser, AST, diagnostics, include handling, and AST-to-structured normalization. |
| `netpilot-filter` | BIRD2 filter language AST, type checker, compiler, VM/interpreter, policy trace, and hit counters. |
| `netpilot-rib` | Route table model, nettypes, route entries, route attributes, best-route selection, next-hop resolution, ECMP. |
| `netpilot-channel` | Protocol channel bindings, import/export filter execution, reload scoping, route event fanout. |
| `netpilot-kernel` | Linux netlink interface, kernel route import/export, interface/address/VRF watch support. |
| `netpilot-protocol` | Shared protocol actor traits, lifecycle, event types, status models, counters. |
| `netpilot-proto-*` | Individual protocol crates for BGP, OSPF, RIP, Babel, BFD, RPKI, Static, Direct, Pipe, MRT, BMP, RAdv, EVPN, L3VPN, Bridge, Aggregator, Perf. |
| `netpilotd` | Main all-in-one daemon, REST API, Web static serving, supervisor, config manager wiring. |
| `netpilotctl` | CLI and BIRD-like operational command client. |
| `netpilot-web` | Web UI source consuming REST/OpenAPI. |

The initial repository may add these crates gradually. The names are design targets, not a requirement to scaffold every crate at once.

## Routing Table and Nettype Model

BIRD2 uses independent routing tables, each holding routes of one nettype. NetPilot follows that model.

### Required Nettypes

| Nettype | Primary key |
| --- | --- |
| `ipv4` | Destination IPv4 prefix. |
| `ipv6` | Destination IPv6 prefix. |
| `ipv6_sadr` | Destination IPv6 prefix plus source IPv6 prefix. |
| `vpn4` | IPv4 prefix plus route distinguisher. |
| `vpn6` | IPv6 prefix plus route distinguisher. |
| `roa4` | Prefix, max length, origin AS. |
| `roa6` | Prefix, max length, origin AS. |
| `aspa` | Customer AS and provider AS set. |
| `flow4` | IPv4 flow specification key. |
| `flow6` | IPv6 flow specification key. |
| `eth` | MAC address plus VLAN id. |
| `mpls` | MPLS label. |
| `evpn` | EVPN route-type-specific key. |
| `neighbor` | IP address plus interface index. |

### Route Record

Every route entry stores:

- Table id and nettype.
- Primary key.
- Source protocol instance.
- Source channel.
- Route preference.
- Next-hop set with optional interface, gateway, MPLS label stack, and weight.
- Common attributes.
- Protocol-specific attributes.
- Lifecycle state: active, hidden, filtered, stale, withdrawn, rejected, unreachable.
- Timestamps, generation id, and revision/event source.

### Best-Route Selection

The global selection order follows the BIRD2 model:

1. Compare route preference.
2. Compare source protocol instance preference.
3. If source protocols are the same type, invoke protocol-specific selection.
4. If protocol types differ after preference comparison, keep behavior deterministic but document it as NetPilot-defined.

Protocol-specific algorithms live inside protocol crates. The RIB provides ordering hooks and stores suboptimal routes.

## Protocol and Channel Model

Each protocol instance implements a common actor contract:

```text
Protocol Actor
  receives config snapshots and scoped reload commands
  receives dependency events such as interface changes or BFD state
  emits route announcements and withdrawals
  emits protocol status, counters, logs, and alarms
  accepts operational commands such as enable, disable, restart, reload, reset
```

Each channel connects one protocol instance to one table. Channel responsibilities:

- Import routes from protocol to table.
- Export routes from table to protocol.
- Apply import/export filters.
- Keep filter hit counters and trace samples.
- Preserve channel reload boundaries.
- Support channel-level options, nettype selection, and table binding.

BFD and Device are service actors without route channels. They publish dependency events for other actors.

## BIRD2 Configuration Compatibility

NetPilot must support both native structured config and BIRD2-compatible text config.

```text
BIRD2 config text
  -> lexer
  -> parser
  -> AST with source spans
  -> semantic analyzer
  -> normalized structured config
  -> diff
  -> commit
```

### Parser Requirements

- Support BIRD2 global options, table declarations, protocol blocks, templates, channels, filters, functions, constants, sets, and include directives.
- Preserve source spans for high-quality errors in CLI and Web.
- Support parse-only mode equivalent to `bird -p`.
- Normalize compatible syntax into NetPilot structured config without losing semantics.
- Report unsupported constructs explicitly with compatibility level, not vague parser errors.

### Include Handling

The parser supports `include` with:

- Configurable root directory.
- Glob support matching BIRD2 expectations where applicable.
- Cycle detection.
- Stable ordering.
- Source mapping across included files.

### Native and Compatible Modes

NetPilot supports three input modes:

| Mode | Behavior |
| --- | --- |
| Native | JSON/YAML/TOML-like structured config edited by API/Web. |
| BIRD2 compatible | `bird.conf` text parsed and normalized into structured config. |
| Mixed | Web edits structured config generated from BIRD2 input, then exports back to BIRD2-compatible text where round-trip support is available. |

Round-trip formatting is desirable but not required for the first parser milestone. Semantic import is required.

## Filter and Policy Compatibility

BIRD2 filters are a central compatibility requirement. NetPilot implements a constrained filter VM.

```text
Filter source
  -> parser
  -> AST
  -> type checker
  -> compiler
  -> bytecode or IR
  -> VM execution
  -> decision trace and counters
```

Required filter capabilities:

- Data types: bool, int, pair, quad, string, prefix, IP address, enum-like symbols, clist/eclist/lclist, prefix sets, pair sets, EC sets, LC sets, and protocol-specific attributes.
- Operators and expressions compatible with BIRD2 where possible.
- Control structures: if, case-like selection where supported, function calls, return, accept, reject.
- Route attribute read/write for common attributes and protocol-specific attributes.
- Set matching for prefixes, AS paths, communities, extended communities, large communities, ROA/ASPA validation state, and protocol metadata.
- Execution limits to prevent infinite loops or runaway policies.
- Dry-run and trace mode for Web/API.

Filter compatibility must be tested with golden BIRD2 configs and route samples.

## Configuration Workflow

NetPilot keeps the candidate/running model from Milestone 1 and extends it for BIRD2 parity.

```text
edit candidate
  -> parse native or BIRD2 config
  -> normalize
  -> schema validate
  -> semantic validate
  -> dependency check
  -> diff
  -> risk classification
  -> commit transaction
  -> scoped actor reloads
  -> convergence/health check
  -> mark success or rollback
```

Commit features:

- Commit notes and user identity.
- Revision id, full snapshot, structured diff, source format, and source text where applicable.
- Confirmed commit with automatic rollback.
- Rollback to a previous revision.
- Parse-only and validate-only commands.
- Import existing `bird.conf` into candidate config.
- Export running config to native structured form and BIRD2-compatible text where supported.

High-risk diffs include:

- Removing many BGP neighbors.
- Clearing export policy.
- Changing router id.
- Changing ASN.
- Replacing kernel export filters.
- Changing table-to-kernel binding.
- Restarting OSPF or BGP instances.
- Changing RPKI validation policy from strict to permissive.

## Reconfiguration Semantics

NetPilot should match BIRD2's operational expectation that config changes do not require daemon restart.

Reload behavior:

- Pure filter changes use soft reload when safe.
- Channel binding changes reload only affected channels.
- Neighbor-level BGP changes reload affected sessions where possible.
- Protocol identity changes restart affected protocol instances.
- Table nettype changes require table replacement and dependent protocol reload.
- Kernel sync changes apply through route diffs, never by blind table flushing unless explicitly requested.

Every reload decision is visible in diff preview.

## Remote Control and CLI

NetPilot exposes both native and BIRD-like operations.

### `netpilotctl`

Required command families:

- Show status, protocols, interfaces, route tables, route details, symbols, memory/performance data.
- Configure, parse, check, reload, commit, confirmed commit, confirm, rollback.
- Enable, disable, restart, reload, and reset protocols or sessions.
- Show route filtered/hidden/exported/imported where the data exists.
- Query BGP neighbor state, routes in/out, last error, capabilities, timers, and counters.
- Query OSPF neighbors, interfaces, LSDB summary, SPF state.
- Query BFD sessions, RPKI caches, kernel sync, MRT/BMP status.

### Control Socket

NetPilot should support a local control endpoint with semantics close to BIRD's control socket. The implementation may use a Unix socket on Linux and a named pipe or TCP loopback endpoint on Windows development builds.

### API Mapping

Every CLI command should map to an internal command API so Web/API and CLI share behavior.

## Web Management Plane

The Web UI is an operations workbench, not a marketing site. It should be dense, status-oriented, and useful during repeated network operations.

Required views:

- Dashboard: router id, ASN, uptime, version, protocol summary, RIB counts, kernel sync state, recent alarms.
- Protocols: list and detail pages for every protocol instance with state, counters, logs, routes, channels, and actions.
- BGP: neighbor table, state, uptime, prefixes received/accepted/exported, capabilities, timers, last error, flap count, route search.
- OSPF: areas, interfaces, neighbors, LSDB summary, SPF runs, route contribution.
- RIB: table browser by nettype, prefix, protocol, state, attributes, hidden/suboptimal routes, selected route.
- Policy: filter editor, structured policy view, dry-run, trace, hit counters, rejected route samples.
- Config: candidate/running diff, import `bird.conf`, validate, commit, confirmed commit, rollback, revision history.
- Kernel: imported/exported routes, sync errors, Linux table ids, VRF/interface binding.
- Diagnostics: logs, event stream, performance counters, memory/route counts, MRT/BMP export state.
- Users/Audit: roles, commit logs, operational commands, rollback events.

The Web UI must not bypass config or command APIs.

## REST and Future gRPC API

REST is the first external API because it is simple for Web UI and automation. gRPC is reserved for future controller/agent mode.

Core REST resources:

- `/api/health`
- `/api/config/running`
- `/api/config/candidate`
- `/api/config/diff`
- `/api/config/parse`
- `/api/config/validate`
- `/api/config/commit`
- `/api/config/confirm`
- `/api/config/rollback`
- `/api/revisions`
- `/api/protocols`
- `/api/protocols/{id}`
- `/api/protocols/{id}/actions`
- `/api/routes`
- `/api/routes/{table}/{nettype}/{key}`
- `/api/policy/trace`
- `/api/kernel`
- `/api/events`
- `/api/audit`

Future controller/agent mode:

```text
controller
  Web UI / external REST / inventory / audit / config distribution

agent
  protocols / RIB / filter VM / kernel sync / local command API

controller <-> agent
  tonic gRPC with typed config, command, event, route summary, and health streams
```

## Storage

First-stage storage:

- File-backed running and candidate snapshots.
- SQLite revision log.
- Structured diff storage.
- Optional source text storage for imported BIRD2 configs.
- Audit events for commits, rollbacks, protocol actions, login events, and high-risk operations.

Later controller storage:

- PostgreSQL or another durable multi-node store for inventory, config distribution, audit, and user management.

## Security and Privileges

NetPilot requires privileged operations on production routers. The daemon should minimize privilege exposure.

Security requirements:

- Run with the smallest practical Linux capabilities.
- Separate privileged kernel/raw-socket operations into narrow adapter modules.
- Support dedicated runtime user and group.
- Protect control endpoints with filesystem permissions, local auth, or reverse-proxy auth.
- Roles: admin, operator, viewer.
- Audit all config commits, rollbacks, protocol resets, restarts, export changes, and user actions.
- Do not let filter code perform file I/O, network access, process execution, or unrestricted loops.

## Linux and Platform Target

The first production target is Linux server/cloud router deployments.

Required Linux features:

- Netlink route import/export.
- Interface/address/link watch.
- VRF awareness where available.
- Multiple kernel routing table ids.
- IPv4 and IPv6 forwarding compatibility.
- Raw socket support for protocols that need it.
- Network namespace based test environments.

BIRD2 also supports BSD platforms. NetPilot may add BSD adapters later, but Linux parity comes first.

## Observability

NetPilot must expose:

- Structured logs.
- Protocol event stream.
- Route change stream.
- RIB counts by table, nettype, protocol, and state.
- Filter hit counters and trace samples.
- BGP/OSPF/BFD/RPKI specific counters.
- Kernel sync success/failure counters.
- Performance counters equivalent to or better than BIRD2 Perf visibility.
- Prometheus-compatible metrics as a later API surface.

## Testing Strategy

Testing must prove compatibility, not only internal correctness.

Required test layers:

- Unit tests for parsers, AST normalization, validation, diff, RIB selection, route attributes, and filter VM.
- Golden tests using BIRD2 config snippets and expected NetPilot structured config.
- Golden tests for BIRD2 filter behavior with sample routes.
- Protocol state-machine tests for BGP, OSPF, RIP, Babel, BFD, RPKI, and others.
- Wire-format tests using captured protocol packets where practical.
- Linux network namespace integration tests for Static, Direct, Kernel, BGP, OSPF, RIP, BFD, and RPKI scenarios.
- Interoperability tests against real BIRD2 and other routing daemons where practical.
- Web/API tests for config import, diff, commit, rollback, route search, and protocol actions.
- Regression tests for every compatibility matrix entry before marking it Level 3 or Level 4.

## Compatibility Acceptance Rules

A feature is not marked complete until:

- It has a matrix entry.
- It has parser/config coverage where applicable.
- It has runtime tests.
- It has operational CLI/API visibility.
- It has Web visibility if operators naturally expect it.
- Its unsupported options are documented.
- It has at least one migration example when it touches BIRD2 config syntax.

For critical routing behavior, "works in one happy path" is not enough. The test must cover reload, withdrawal, error handling, and observability.

## Implementation Roadmap

### Phase 1: Compatibility Inventory and Core Model

- Expand the design and matrix.
- Add route nettypes and table model.
- Add protocol/channel abstractions.
- Add route attribute model.
- Add compatibility status tracking in docs/tests.

### Phase 2: RIB, Static, Direct, Device, Kernel

- Implement the route table core.
- Implement Static routes.
- Implement Device interface watcher.
- Implement Direct route discovery.
- Implement Kernel import/export through Linux netlink.
- Prove route-to-kernel and kernel-to-route loops.

### Phase 3: BIRD2 Config Parser Foundation

- Lexer, parser, source spans, include handling.
- Global options, tables, static/direct/kernel/device protocol blocks.
- Parse-only and validate-only endpoints.
- Golden config fixtures.

### Phase 4: Filter Language and Policy VM

- Filter AST, type checker, compiler/interpreter.
- Common route attributes.
- Prefix/community/as-path set support.
- Import/export channel integration.
- Policy trace and Web dry-run.

### Phase 5: BGP

- BGP transport, OPEN/KEEPALIVE/UPDATE/NOTIFICATION.
- IPv4/IPv6 unicast first.
- Route attributes, communities, extended communities, large communities.
- Import/export filters, route refresh, graceful restart.
- Route server behavior and RPKI hooks.

### Phase 6: BFD, RPKI, MRT, BMP

- BFD service actor and protocol subscriptions.
- RPKI RTR client and validation state.
- MRT dump support.
- BMP monitoring export.

### Phase 7: OSPF, RIP, Babel, RAdv

- OSPFv2/v3 with LSDB and SPF.
- RIPv2/RIPng.
- Babel including source-specific route support where needed.
- IPv6 Router Advertisements.

### Phase 8: Advanced Route Families

- MPLS route support.
- VPN4/VPN6 and L3VPN.
- Flow4/Flow6.
- EVPN.
- ETH/Bridge.
- Aggregator.
- Perf diagnostics.

### Phase 9: Operational Compatibility

- `netpilotctl` and local control endpoint.
- BIRD-like show/configure/reload commands.
- Web parity dashboards for every protocol.
- Migration tooling from existing `bird.conf`.

### Phase 10: Controller/Agent Mode

- Agent gRPC API.
- Controller inventory.
- Multi-node config distribution.
- Central audit and revision history.

## Near-Term Next Plan

The next implementation plan should not try to implement all BIRD2 features at once. It should build the foundation required for later parity:

1. Create `netpilot-rib` with nettypes, route entries, tables, and best-route selection.
2. Create shared protocol/channel traits.
3. Extend structured config with tables, channels, Static, Direct, Device, and Kernel.
4. Add compatibility matrix tests that start as expected failures or inventory checks.
5. Keep Web/API config workflow working while adding RIB visibility endpoints.

This keeps the project moving while protecting the full BIRD2 target from becoming an untestable monolith.

## Open Decisions

- Exact first target operating system for production testing: Linux distribution and kernel minimum.
- Whether native structured config should prefer JSON, YAML, or TOML on disk.
- Whether `netpilotctl` should intentionally mimic `birdc` command text or provide a cleaner native command set plus aliases.
- Exact BIRD2 syntax subset for the first parser milestone.
- Whether BSD support is a hard requirement after Linux parity or a later optional adapter.

## Feature Gap Analysis (vs BIRD2 2.19.1 + FRR)

> This section added 2026-06-12 after cross-referencing NetPilot's 233-feature list against BIRD 2.19.1 official documentation, FRR documentation, and modern network operator requirements. 100 additional features identified.

### Gap Category A: Missing BIRD2 Configuration System Options

| # | Feature | Source |
|---|---------|--------|
| 234 | Channel route limits: `import limit` / `receive limit` / `export limit` with action (warn/block/restart/disable) | BIRD2 |
| 235 | Channel `import keep filtered`: retain routes rejected by import filter, queryable via `show route filtered` | BIRD2 |
| 236 | Channel `rpki reload`: auto-reevaluate routes on ROA/ASPA changes | BIRD2 |
| 237 | Table GC options: `gc threshold` (min routes to trigger GC), `gc period` (adaptive 10-600s) | BIRD2 |
| 238 | Table `sorted` / `trie` options: sorted route list guarantee, trie-accelerated lookups | BIRD2 |
| 239 | Table `min settle time` / `max settle time`: RPKI reload stabilization windows | BIRD2 |
| 240 | MPLS domain configuration: `mpls domain <name> { label range { start; length; } }` — independent label space management | BIRD2 |
| 241 | MPLS channel options: `domain`, `label range`, `label policy` (static/prefix/aggregate/vrf) | BIRD2 |
| 242 | MPLS max stack depth: compile-time limit (BIRD2 default 8 labels) | BIRD2 |
| 243 | Password/authentication system: multi-password with id, time windows (generate from/to, accept from/to), algorithms (HMAC-SHA256/384/512, BLAKE2s/b) | BIRD2 |
| 244 | `tx class/dscp`: IP header ToS/DS/DSCP marking (default 0xc0) | BIRD2 |
| 245 | `tx priority`: local packet priority via Linux SO_PRIORITY (default 7) | BIRD2 |
| 246 | Protocol `description`: human-readable description text per protocol instance | BIRD2 |
| 247 | `router id from [...]`: auto-select router-id from interface address pattern | BIRD2 |
| 248 | `hostname`: global hostname configuration (defaults to uname -n) | BIRD2 |
| 249 | CLI socket configuration: `cli "<name>"` + `restrict` read-only mode | BIRD2 |
| 250 | `define` constant system: global numeric/string constant definitions | BIRD2 |
| 251 | Custom route attributes: `attribute <type> <name>` for declaring custom attributes | BIRD2 |
| 252 | `watchdog warning/timeout`: I/O loop warning/abort thresholds | BIRD2 |
| 253 | `debug latency` tracking: internal event timing instrumentation | BIRD2 |
| 254 | Multiple time formats: `timeformat route|protocol|base|log` per-subsystem format | BIRD2 |
| 255 | `mpls table` keyword: dedicated MPLS routing table type | BIRD2 |

### Gap Category B: Missing BIRD2 CLI Commands

| # | Feature | Source |
|---|---------|--------|
| 256 | `eval <expr>`: evaluate filter expressions directly in CLI | BIRD2 |
| 257 | `dump resources|sockets|events|interfaces|neighbors|attributes|routes|protocols "<file>"`: export internal state | BIRD2 |
| 258 | `debug <protocols>|<pattern>|all all|off|{states|routes|filters|events|packets}`: dynamic per-protocol debug level | BIRD2 |
| 259 | `down`: graceful daemon shutdown | BIRD2 |
| 260 | `graceful restart`: trigger graceful restart then exit | BIRD2 |
| 261 | `echo all|off|{classes} [buffer-size]`: control log class output | BIRD2 |
| 262 | `timeformat "<fmt1>" [limit "<fmt2>"]`: dynamic time display format switch | BIRD2 |
| 263 | Restricted CLI mode: `birdc -r` read-only operations | BIRD2 |
| 264 | `configure soft`: soft reconfiguration (only reload changed protocols) | BIRD2 |
| 265 | `configure undo`: undo unconfirmed confirmed commit | BIRD2 |
| 266 | `configure timeout <n>`: confirmed commit with auto-rollback timeout | BIRD2 |
| 267 | `show route filter <name>`: debug filter output for a specific filter function | BIRD2 |
| 268 | `show route filtered`: show routes retained by import keep filtered | BIRD2 |

### Gap Category C: Missing BIRD2 Filter Language Features

| # | Feature | Source |
|---|---------|--------|
| 269 | `for` loop: `for [type] <var> in <expr> do <cmd>` — iterate over tables/sets | BIRD2 |
| 270 | `bgppath` type with operations: `.first`, `.last`, `.last_nonaggregated`, `.len`, `.empty`, `.prepend(n)`, `.delete(n)`, `.filter(n)` | BIRD2 |
| 271 | `bgpmask` type: AS path matching pattern `[= * 64500 + =]` with wildcards and sets | BIRD2 |
| 272 | `clist`/`eclist`/`lclist` mutable community lists: `.len`, `.empty`, `.add(p)`, `.delete(p)`, `.filter(p)`, `.min`, `.max` | BIRD2 |
| 273 | `bytestring` type: arbitrary byte sequences with `++` concat and `from_hex()` function | BIRD2 |
| 274 | `mac` type: MAC address type with literal notation | BIRD2 |
| 275 | `rd` type: Route Distinguisher type (three forms per RFC 4364) | BIRD2 |
| 276 | `print` / `printn` statements: filter-internal debug output (with/without newline) | BIRD2 |
| 277 | `defined(<attribute>)` function: check if route attribute exists | BIRD2 |
| 278 | `unset(<attribute>)` function: undefine optional route attribute | BIRD2 |
| 279 | `case` full syntax: `case <expr> { <set_expr>: <stmt>; else: <stmt>; }` with set-expression branches | BIRD2 |
| 280 | Typed function system: `function <name>(<params>) -> <type> [vars] { ... }` with explicit return types | BIRD2 |
| 281 | Custom attribute read/write in filter: declare with `attribute <type> <name>`, access in filters | BIRD2 |
| 282 | `gw_mpls` attribute: outgoing MPLS label on route | BIRD2 |
| 283 | `mpls_label` / `mpls_policy` / `mpls_class` attributes: MPLS-related route attributes | BIRD2 |
| 284 | `igp_metric` attribute: IGP metric distance for BGP internal path comparison | BIRD2 |
| 285 | EVPN prefix operators: `.evpn_type`, `.evpn_tag`, `.evpn_esi`, `.router_ip` for EVPN route subtypes | BIRD2 |
| 286 | Prefix nettype constants: `NET_IP4`, `NET_IP6`, `NET_VPN4`, `NET_VPN6`, `NET_EVPN`, `NET_EVPN_EAD`, `NET_EVPN_MAC`, `NET_EVPN_IMET`, `NET_EVPN_ES`, etc. | BIRD2 |

### Gap Category D: Missing Protocol Details (BIRD2 + FRR)

| # | Feature | Source |
|---|---------|--------|
| 287 | BGP `import table` / `export table` syntax: explicit channel table bindings | BIRD2 |
| 288 | BGP Long-Lived Graceful Restart (LLGR): configurable stale time up to 16,777,215 seconds, per-AFI staling | FRR |
| 289 | BGP per-peer GR mode: restarter, helper, disable per neighbor | FRR |
| 290 | BGP update-delay vs advertisement-delay: distinguish FIB programming delay from neighbor advertisement delay | FRR |
| 291 | BGP coalesce-time: update-group batching merge interval (default 1000ms) | FRR |
| 292 | BGP dynamic neighbor listen: `bgp listen range <prefix>` for dynamic peer acceptance | FRR |
| 293 | BGP-LU (Labeled Unicast, SAFI 4): RFC 3107 / RFC 8277 | FRR |
| 294 | BGP multi-AS per daemon via VRF: single bgpd process running multiple autonomous systems | FRR |
| 295 | BGP views: independent routing tables not installed to kernel (route-server use case) | FRR |
| 296 | BGP link-bandwidth extended community: IEEE float or uint32 encoded link bandwidth | FRR |
| 297 | OSPF NSSA detailed support: NSSA-LSA type 7, translate-always/translate-never/candidate | BIRD2 |
| 298 | OSPF template/from inheritance: BIRD2 notes this as "not implemented for OSPF" — NetPilot should support it | BIRD2 |
| 299 | Static route nexthop types complete: blackhole, unreachable, prohibit (not just blackhole) | BIRD2 |
| 300 | RPKI ASPA complete checks: `aspa_check_downstream(table)`, `aspa_check_upstream(table)` in filters | BIRD2 |

### Gap Category E: FRR-Unique Protocols/Services (not in BIRD2)

| # | Feature | Source |
|---|---------|--------|
| 301 | IS-IS protocol: IGP widely used in enterprise/SP networks. FRR supports, BIRD2 does not | FRR |
| 302 | PIM / PIMv6 (Multicast routing): multicast route distribution. BIRD2 has zero multicast support | FRR |
| 303 | LDP (Label Distribution Protocol): MPLS label distribution. BIRD2 has MPLS forwarding but no LDP | FRR |
| 304 | EIGRP: Cisco proprietary protocol, FRR has basic support | FRR |
| 305 | NHRP (Next Hop Resolution Protocol): DMVPN/hub-spoke scenarios | FRR |
| 306 | PBR (Policy-Based Routing): source/destination/port-based route selection | FRR |
| 307 | VRRP (Virtual Router Redundancy Protocol): gateway HA without proprietary solutions | FRR |
| 308 | SBFD (Seamless BFD): stateless BFD for large-scale deployments | FRR |
| 309 | OpenFabric: IS-IS-derived fabric protocol | FRR |
| 310 | BGP-LS (BGP Link State): topology export to controllers (draft-ietf-idr-ls-distribution) | FRR |
| 311 | BGPsec: BGP path validation security extensions | Industry |
| 312 | SNMP support: SNMP agent with routing MIBs | FRR |
| 313 | YANG models (IETF Routing YANG): RFC 8349, RFC 8294, and protocol-specific YANG modules | FRR mgmtd |
| 314 | NETCONF / RESTCONF: standardized configuration protocols | FRR mgmtd |
| 315 | gNMI streaming telemetry: gRPC Network Management Interface for real-time state push | FRR mgmtd |
| 316 | gRPC northbound interface: protobuf-based management API (FRR already implemented) | FRR |
| 317 | Segment Routing — SR-MPLS: IGP-based source routing with label stacks | FRR |
| 318 | Segment Routing — SRv6: IPv6-based segment routing | FRR |
| 319 | BGP Flowspec (SAFI 133/134): BGP-based traffic filtering and policy distribution | FRR |
| 320 | VNC (Virtual Network Controller): SDN controller integration virtual network | FRR |

### Gap Category F: Modern Networking Platform Requirements (2026)

| # | Feature | Source |
|---|---------|--------|
| 321 | eBPF/XDP high-performance forwarding: kernel-level line-rate packet processing | Industry |
| 322 | Kubernetes CNI integration: Calico/Cilium-compatible CNI plugin | Industry |
| 323 | Kubernetes CRD controller: manage routing config via K8s Custom Resources | Industry |
| 324 | Ansible module: automated deployment and configuration management | Industry |
| 325 | Terraform provider: Infrastructure-as-Code support | Industry |
| 326 | NetBox / CMDB integration: auto-discovery and sync of network assets | Industry |
| 327 | NAT64 / 464XLAT: IPv6 transition technologies | Industry |
| 328 | BGP Flow Specification complete: SAFI 133 (IPv4 flowspec) + 134 (IPv6 flowspec) | Industry |
| 329 | HTTP/3 (QUIC) API: API transport over QUIC for reduced connection latency | Industry |
| 330 | WASM policy plugins: custom filter/policy as WebAssembly sandboxed modules | Industry |
| 331 | GPU-accelerated route processing: GPU offload for route computation and policy evaluation | Industry |
| 332 | Controller/SDN northbound SDK: gRPC + protobuf-based controller SDK | Industry |
| 333 | Chaos engineering hooks: protocol fault injection for resilience testing | Industry |

### Gap Summary

| Category | Count | Priority |
|----------|-------|----------|
| A: BIRD2 Config Options | 22 (#234-#255) | P1 |
| B: BIRD2 CLI Commands | 13 (#256-#268) | P0-P1 |
| C: BIRD2 Filter Language | 18 (#269-#286) | P0 |
| D: Protocol Details | 14 (#287-#300) | P1 |
| E: FRR-Unique Protocols | 20 (#301-#320) | P2 |
| F: Modern Platform Needs | 13 (#321-#333) | P3 |
| **Total** | **100** | — |

**Overall NetPilot feature count: 233 original + 100 gap additions = 333 total features.**

## Explicit Non-Goals

These are non-goals for the next implementation plan only, not for the long-term product:

- Implementing every BIRD2 protocol in one milestone.
- Full controller/agent split before the single-node daemon is useful.
- Dynamic binary plugin loading.
- Hardware router ASIC programming.
- A byte-for-byte clone of BIRD internals.

Long term, BIRD2 functional and configuration compatibility remains in scope. All 333 features above are part of the long-term roadmap.
