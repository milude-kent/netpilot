# NetPilot M6 — IS-IS Protocol Design

Date: 2026-06-13

## Goal

Implement IS-IS routing protocol (#301) as the first complete protocol using the ProtocolActor framework. Full configuration schema, adjacency state machine (Down/Init/Up), LSP database, SPF computation, packet structures (IIH/LSP/CSNP/PSNP), TLV parser/builder (13 types), raw socket transport, and SR extensions.

## Scope

### In scope

| Component | Description |
|-----------|-------------|
| IsisConfig + IsisInterfaceConfig | Full BIRD2/FRR-compatible schema, new variant in ProtocolConfig |
| IsisActor | ProtocolActor impl with select! event loop (commands, packets, timers, interfaces) |
| Adjacency FSM | Down→Init→Up→Down via hello processing and holding timer |
| LSP database | Insert, query, expire, self-LSP generation, flood |
| SPF (Dijkstra) | Pure function: compute routes from LSP DB |
| Packet types | IIH, LSP, CSNP, PSNP with wire-format headers |
| TLV parser/builder | 13 TLV types: AreaAddresses(1), IsNeighbors(2), ExtendedReachability(22), IpReach(128/130), Ipv6Reach(236), Hostname(137), RouterCap(242), SrCap(242), PrefixSid(235), AdjacencySid(240), LanId(6), Protocols(129) |
| IsisTransport trait | Abstract send/recv + raw socket impl (socket2) |
| Timers | Hello interval, LSP refresh, CSNP interval, holding timer expiry |
| DIS election | Priority comparison on broadcast interfaces |
| SR extensions | TLV 242 (SR-Cap), TLV 235 (Prefix-SID), TLV 240 (Adjacency-SID) |
| CLI | show isis topology, show isis adjacencies, show isis database |
| Tests | Adjacency FSM, SPF, TLV round-trip |

### Out of scope

- Summary addresses / route leaking
- IS-IS authentication
- Multi-topology IS-IS for IPv6
- BFD integration

## Architecture

```
netpilot-proto-isis/
  src/lib.rs           ← crate root + re-exports
  src/config.rs         ← IsisConfig, IsisInterfaceConfig, CircuitType, IsisLevel
  src/actor.rs          ← IsisActor: ProtocolActor impl
  src/adjacency.rs      ← Adjacency FSM, holding timer
  src/lsp.rs            ← LspDatabase, LspEntry
  src/spf.rs            ← compute_spf() pure function
  src/packet.rs         ← IsisPacket, IsisHeader, IihPacket, LspPacket, CsnpPacket, PsnpPacket
  src/tlv.rs            ← IsisTlv enum, parse_tlvs(), build_tlvs()
  src/timer.rs          ← Hello/LSP/CSNP intervals
  src/transport.rs      ← IsisTransport trait + RawSocketTransport

Dependencies: netpilot-protocol, netpilot-config, netpilot-kernel, tokio, socket2, thiserror
```

## Key Data Flow

```
Config commit → ProtocolSupervisor.spawn(IsisActor)
  → IsisActor::run()
    → Main loop: select! {
        rx.recv()            → handle daemon commands
        transport.recv()     → process IS-IS packets
        timer.tick()         → send hello, refresh LSPs
        watcher.watch()      → handle interface up/down
      }
    → On route change:
        compute_spf(lsp_db, system_id, level)
        → SpfResult.routes
          → tx.send(ProtocolEvent::RouteAnnounce { ... })
```

## Files Changed

| File | Approx lines |
|------|-------------|
| `crates/netpilot-proto-isis/Cargo.toml` | +20 |
| `crates/netpilot-proto-isis/src/lib.rs` | +30 |
| `crates/netpilot-proto-isis/src/config.rs` | +80 |
| `crates/netpilot-proto-isis/src/actor.rs` | +200 |
| `crates/netpilot-proto-isis/src/adjacency.rs` | +120 |
| `crates/netpilot-proto-isis/src/lsp.rs` | +150 |
| `crates/netpilot-proto-isis/src/spf.rs` | +100 |
| `crates/netpilot-proto-isis/src/packet.rs` | +120 |
| `crates/netpilot-proto-isis/src/tlv.rs` | +200 |
| `crates/netpilot-proto-isis/src/timer.rs` | +80 |
| `crates/netpilot-proto-isis/src/transport.rs` | +150 |
| `crates/netpilot-proto-isis/tests/*.rs` | +240 |
| `crates/netpilot-config/src/schema.rs` | +60 |
| `crates/netpilotd/src/cli.rs` | +25 |
| `Cargo.toml` | +3 |
| **Total** | **~1578** |
