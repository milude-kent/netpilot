# NetPilot — Complete Audit Report (2026-06-13)

## Summary
- **205 tests, 0 failures**
- **22 crates, ~14,000 lines Rust**
- **12 P0 blockers fixed, 0 remaining**
- **Overall maturity: 60% prototype to 75% after P0 fixes**

## Protocol Completeness

| Protocol | Config | Actor | Transport | Packet I/O | Route Engine | Overall |
|----------|--------|-------|-----------|------------|--------------|---------|
| BGP | 95% | 80% | TCP real | 90% | N/A | 85% |
| IS-IS | 80% | 85% | Loopback | 70% | SPF real | 75% |
| EIGRP | 80% | 70% | Loopback | 30% | DUAL partial | 60% |
| OSPF | 80% | 40% | None | 0% | SPF real | 50% |
| Static | 100% | 100% | N/A | N/A | N/A | 100% |
| RIP | 60% | 20% | None | 0% | DV stub | 25% |
| LDP | 60% | 15% | None | 0% | Label bind | 20% |
| PIM | 60% | 10% | None | 0% | None | 15% |
| BFD | 50% | 12% | None | 0% | N/A | 15% |
| RPKI | 60% | 15% | None | 0% | ROA hash | 20% |

**Legend:**
- **Config**: Schema coverage + validation completeness
- **Actor**: Protocol actor implementation maturity
- **Transport**: Real vs loopback/test transport layer
- **Packet I/O**: Wire-format encode/decode completeness
- **Route Engine**: Algorithm implementation (SPF, DUAL, best-path)
- **Overall**: Weighted average of above dimensions

## P0 Fixes Applied

| # | Fix | Crate(s) | Impact |
|---|-----|----------|--------|
| 1 | main.rs now spawns protocol actors from running config | netpilotd | Critical — daemon startup was broken; no protocols would run |
| 2 | Supervisor event_tx wired to all 9 protocol actors | netpilotd, netpilot-protocol | Critical — route announcements were never broadcast |
| 3 | BGP OPEN decode byte offset corrected (hold_time, bgp_identifier) | netpilot-proto-bgp | Critical — BGP sessions would fail OPEN negotiation |
| 4 | BGP persistent session loop with KEEPALIVE + reconnect | netpilot-proto-bgp | Critical — sessions were one-shot, no keepalive, no reconnect |
| 5 | BGP UPDATE message decode support added | netpilot-proto-bgp | Critical — could not process route updates from peers |
| 6 | Kernel route messages now set real prefix/gateway/metric | netpilot-kernel | Critical — FIB installs had garbage data |
| 7 | RIB-to-Kernel FIB sync wired in event processor | netpilotd, netpilot-rib | Critical — routes never reached the kernel |
| 8 | IS-IS packet encode/decode functions added | netpilot-proto-isis | High — could not send or parse IS-IS packets |
| 9 | IS-IS LoopbackTransport + runtime injectable transport | netpilot-proto-isis | High — IS-IS had no transport layer |
| 10 | IS-IS hold timer decrement + adjacency expiry detection | netpilot-proto-isis | High — adjacencies never expired, memory leak |
| 11 | EIGRP EigrpTransport trait + LoopbackTransport | netpilot-proto-eigrp | High — EIGRP had no transport abstraction |
| 12 | OSPF real Dijkstra SPF replacing empty stub | netpilot-proto-ospf | High — OSPF route computation was a no-op |

## What Changed: Before vs After

### Before P0 Fixes
- `cargo run` produced a daemon that listened on ports but ran no protocols
- Protocol actors received no startup signals — all 9 were inert
- BGP OPEN decode was reading wrong byte offsets, corrupting field values
- No UPDATE decode meant BGP peers could not exchange routes
- Kernel FIB installs had uninitialized memory in prefix/gateway fields
- RIB computed best routes but never triggered kernel writes
- IS-IS had no encode/decode, no transport, broken hold timer
- EIGRP had no transport layer, messages were silently dropped
- OSPF SPF returned an empty route set

### After P0 Fixes
- Daemon startup reads running config and spawns all configured protocols
- Supervisor broadcast channel carries RouteAnnounce events to all subscribers
- BGP establishes TCP sessions with correct OPEN handling, maintains KEEPALIVE
- BGP UPDATE messages are correctly decoded with NLRI and path attributes
- Kernel route entries carry actual prefix, gateway, and metric
- RIB changes trigger FIB installs/deletes transparently
- IS-IS can encode/decode all packet types, uses loopback transport for testing
- IS-IS adjacency handling correctly tracks hold timers
- EIGRP has transport trait with loopback implementation
- OSPF SPF computes real routes from LSDB, including ECMP paths

## Remaining Gaps

### Architecture
- **Filter parser (text-to-AST)**: The filter VM (interpreter) works, but there is no parser to convert BIRD2 filter expressions into the VM bytecode. Users cannot write filters today.
- **drop_privileges**: Implemented as a no-op on all platforms. The daemon always runs as the invoking user.
- **Schema gap: BGP neighbor tuning fields**: ~20 commonly used BGP neighbor fields (hold_time, keepalive, next_hop_self, etc.) are not in the configuration schema.

### Management Interfaces
- **gNMI streaming**: Subscribe is limited to 5 hardcoded paths; no real dynamic telemetry streaming.
- **BIRD config parser**: ~40% syntax coverage. Handles static/BGP/OSPF blocks but missing many filter, function, and advanced syntax features.
- **CLI completeness**: Many show commands parsed but backend implementations are stubs.

### Protocol Actors
- **LDP, PIM, RIP, BFD, RPKI actors**: Skeleton implementations. Have config parsing and actor struct but no protocol logic — no hello exchange, no adjacency FSM, no route computation.
- **OSPF**: Has SPF algorithm but no real network transport (no OSPF Hello exchange, no LSDB synchronization).
- **EIGRP**: Has DUAL algorithm but no real packet exchange with neighbors.

### Testing
- **Integration tests**: Basic smoke tests exist but no multi-protocol convergence tests, no fault-injection tests, no performance benchmarks.
- **MacOS CI**: Tests pass on macOS but kernel-dependent tests (netlink) are skipped — no coverage of actual kernel integration.

## Recommendations

1. **Immediate (next sprint)**:
   - Add BGP neighbor tuning fields to schema (hold_time, keepalive, next_hop_self)
   - Implement filter expression parser (text-to-AST)
   - Wire OSPF transport layer (raw socket + hello protocol)

2. **Short-term (2-3 sprints)**:
   - Complete BIRD config parser to 70%+ syntax coverage
   - Add gNMI dynamic path registration for streaming
   - Build LDP adjacency + hello exchange (foundation for MPLS forwarding)

3. **Medium-term (4-6 sprints)**:
   - RIP full implementation (simple protocol, good for completeness)
   - Integration test suite with multi-protocol convergence scenarios
   - Performance benchmarks for RIB operations at 1M+ routes

4. **Long-term**:
   - PIM-SM full implementation with RPF, assert, and DR election
   - BFD protocol actor with sub-second failure detection
   - RPKI RTR client with incremental update (RFC 8210)
