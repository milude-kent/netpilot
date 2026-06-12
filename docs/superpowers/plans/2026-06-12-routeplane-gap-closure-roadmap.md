# RoutePlane Gap Closure — Master Roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the 100 identified feature gaps against BIRD2, FRR, and modern network operator requirements across 6 milestones.

**Architecture:** Features are grouped by priority and dependency into 6 independent milestone plans. Each milestone produces a working, testable artifact. Milestones are sequential but each contains internally parallelizable tasks.

**Tech Stack:** Rust 2024 edition, tokio, axum, serde, thiserror, time. New crates added as needed.

---

## Milestone Map

| Milestone | Priority | Features | Target |
|-----------|----------|----------|--------|
| M2: Filter Language Foundation | P0 | Filter type system, bgppath, clist, for loop, case, functions, defined/unset, print | Week 1-2 |
| M3: BIRD2 Operational Parity | P0-P1 | CLI commands (eval, dump, debug, down, GR, configure soft/undo), config options (channel limits, keep filtered, rpki reload, table GC, password system) | Week 3-4 |
| M4: Protocol Depth & BGP Features | P1 | BGP-LU, LLGR, per-peer GR, coalesce-time, listen range, multi-AS/VRF, BGP views, OSPF NSSA, Static nexthop types, RPKI ASPA | Week 5-7 |
| M5: MPLS & Advanced Config | P1-P2 | MPLS domain, MPLS channel, MPLS table, label policy, label range, SR-MPLS, SRv6 | Week 8-10 |
| M6: FRR-Unique Protocols | P2 | IS-IS, PIM, LDP, BGP-LS, BGP Flowspec, SBFD, SNMP, YANG, NETCONF, gNMI | Week 11-16 |
| M7: Modern Platform & Ecosystem | P3 | eBPF/XDP, K8s CNI/CRD, Ansible, Terraform, NetBox, WASM plugins, GPU offload, controller SDK | Week 17-22 |

---

## Milestone 2: Filter Language Foundation (P0 — 18 features: #269-#286)

### Overview

This milestone implements the complete BIRD2 filter language type system, control structures, and debugging aids. Without this, RoutePlane cannot claim filter compatibility.

### Plan reference: `docs/superpowers/plans/YYYY-MM-DD-routeplane-filter-language.md`

### Task Outline

**Task 1: Extended Type System**
- Written plan: #269-#275 (7 types)

**Task 2: Control Flow & Debugging**
- Written plan: #269 (for loop), #276 (print/printn), #279 (case syntax)

**Task 3: Introspection & Attribute System**
- Written plan: #277 (defined), #278 (unset), #280 (typed functions), #281 (custom attributes)

**Task 4: Protocol Attribute Extensions**
- Written plan: #282 (gw_mpls), #283 (mpls_label/policy/class), #284 (igp_metric), #285 (EVPN operators), #286 (nettype constants)

**Task 5: Golden Filter Tests**
- Written plan: #90 (extends existing golden test plan)

---

## Milestone 3: BIRD2 Operational Parity (P0-P1 — 35 features: #234-#268)

### Overview

Brings RoutePlane's configuration system and CLI to parity with BIRD2's operational commands and configuration depth.

### Plan reference: `docs/superpowers/plans/YYYY-MM-DD-routeplane-operational-parity.md`

### Task Outline

**Task 1: Channel Enhancements**
- #234 (route limits), #235 (import keep filtered), #236 (rpki reload)

**Task 2: Table Management**
- #237 (GC), #238 (sorted/trie), #239 (settle time)

**Task 3: Authentication System**
- #243 (multi-password with time windows and algorithm negotiation)

**Task 4: Protocol Configuration Depth**
- #244 (tx class/dscp), #245 (tx priority), #246 (description), #247 (router-id from), #248 (hostname), #250 (define), #251 (custom attributes), #252 (watchdog), #253 (debug latency), #254 (time formats)

**Task 5: CLI Commands — Config Family**
- #256 (eval), #258 (debug), #259 (down), #260 (graceful restart), #261 (echo), #262 (timeformat), #263 (restricted CLI), #264 (configure soft), #265 (configure undo), #266 (configure timeout)

**Task 6: CLI Commands — Show Family**
- #257 (dump), #267 (show route filter), #268 (show route filtered)

**Task 7: CLI Socket Infrastructure**
- #249 (CLI socket with restrict mode)

---

## Milestone 4: Protocol Depth & BGP Advanced Features (P1 — 14 features: #287-#300)

### Plan reference: `docs/superpowers/plans/YYYY-MM-DD-routeplane-protocol-depth.md`

### Task Outline

**Task 1: BGP Channel & Table Binding**
- #287 (import table / export table)

**Task 2: BGP Graceful Restart Advanced**
- #288 (LLGR), #289 (per-peer GR mode)

**Task 3: BGP Performance & Scale**
- #290 (update-delay/advertisement-delay), #291 (coalesce-time), #292 (listen range)

**Task 4: BGP Multi-AS & Views**
- #293 (BGP-LU / Labeled Unicast), #294 (multi-AS per daemon), #295 (BGP views), #296 (link-bandwidth ext-community)

**Task 5: OSPF & Static Depth**
- #297 (NSSA), #298 (OSPF template), #299 (static nexthop: unreachable, prohibit)

**Task 6: RPKI Depth**
- #300 (ASPA downstream/upstream checks)

---

## Milestone 5: MPLS & Segment Routing (P1-P2 — 10 features: #240-#242, #255, #317-#318 + 4 MPLS features)

### Plan reference: `docs/superpowers/plans/YYYY-MM-DD-routeplane-mpls-sr.md`

### Task Outline

**Task 1: MPLS Foundation**
- #240 (MPLS domain), #241 (MPLS channel), #242 (label stack depth), #255 (MPLS table)

**Task 2: MPLS Label Management**
- Label range allocation, dynamic/static label pools, FEC-to-label binding

**Task 3: Segment Routing — SR-MPLS**
- #317: IGP SR extension, prefix-SID, adjacency-SID, label stack computation

**Task 4: Segment Routing — SRv6**
- #318: SRv6 SID types, SRH insertion, locator management

---

## Milestone 6: FRR-Unique Protocols (P2 — 20 features: #301-#320)

### Plan reference: `docs/superpowers/plans/YYYY-MM-DD-routeplane-frr-protocols.md`

### Task Outline

**Task 1: IS-IS**
- #301: IS-IS levels, areas, LSPs, SPF, TE extensions

**Task 2: Multicast — PIM/PIMv6**
- #302: PIM-SM, PIM-SSM, IGMP, MLD, multicast RIB

**Task 3: MPLS Signaling — LDP**
- #303: LDP sessions, label binding, LDP-IGP sync

**Task 4: Advanced Forwarding**
- #306 (PBR), #307 (VRRP), #308 (SBFD), #309 (OpenFabric), #305 (NHRP)

**Task 5: BGP Advanced AFI/SAFI**
- #310 (BGP-LS), #311 (BGPsec), #319 (BGP Flowspec)

**Task 6: Enterprise Protocols**
- #304 (EIGRP), #320 (VNC)

**Task 7: Management Interfaces**
- #312 (SNMP), #313 (YANG models), #314 (NETCONF/RESTCONF), #315 (gNMI), #316 (gRPC northbound)

---

## Milestone 7: Modern Platform & Ecosystem (P3 — 13 features: #321-#333)

### Plan reference: `docs/superpowers/plans/YYYY-MM-DD-routeplane-modern-platform.md`

### Task Outline

**Task 1: High-Performance Dataplane**
- #321 (eBPF/XDP forwarding), #331 (GPU acceleration)

**Task 2: Kubernetes Integration**
- #322 (CNI plugin), #323 (CRD controller)

**Task 3: Automation Ecosystem**
- #324 (Ansible module), #325 (Terraform provider), #326 (NetBox integration)

**Task 4: IPv6 Transition**
- #327 (NAT64/464XLAT)

**Task 5: Advanced Traffic Engineering**
- #328 (BGP Flowspec complete)

**Task 6: Extensibility & SDK**
- #329 (HTTP/3 API), #330 (WASM policy plugins), #332 (Controller SDK), #333 (Chaos hooks)

---

## Immediate Next Action

Detailed implementation plan for **Milestone 2: Filter Language Foundation** is the recommended next step, as filter compatibility is the hardest requirement for BIRD2 replacement and touches the most fundamental code.

Would you like me to write the detailed task-by-task implementation plan for Milestone 2 (Filter Language Foundation, #269-#286)?
