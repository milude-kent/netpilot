# NetPilot M6 — IS-IS Protocol Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development

**Goal:** Implement IS-IS routing protocol with full adjacency FSM, LSP database, SPF, packet/TLV types, raw socket transport, and SR extensions — the first complete protocol using the ProtocolActor framework.

**Architecture:** New `netpilot-proto-isis` crate. IsisActor implements ProtocolActor with select! event loop. IsisTransport trait abstracts packet I/O. Config integrated into ProtocolConfig enum. Pure-function SPF for testability.

**Tech Stack:** Rust 2024, tokio, async-trait, socket2, netpilot-protocol

---

### Task 1: Crate Scaffold + Config Types

**Files:** Create `crates/netpilot-proto-isis/Cargo.toml`, `src/lib.rs`, `src/config.rs`; Modify root `Cargo.toml`, `netpilot-config/src/schema.rs`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "netpilot-proto-isis"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
netpilot-protocol = { path = "../netpilot-protocol" }
netpilot-config = { path = "../netpilot-config" }
netpilot-kernel = { path = "../netpilot-kernel" }
async-trait = "0.1"
socket2 = "0.5"
thiserror.workspace = true
tokio.workspace = true
time.workspace = true
```

- [ ] **Step 2: Add to workspace** — add `"crates/netpilot-proto-isis"` to root `Cargo.toml`

- [ ] **Step 3: Write config.rs** with IsisConfig, IsisInterfaceConfig, IsisLevel, CircuitType.

- [ ] **Step 4: Add IsisConfig variant to ProtocolConfig** in netpilot-config schema.rs.

- [ ] **Step 5: Build + commit**

---

### Task 2: Packet + TLV Types

**Files:** Create `src/packet.rs`, `src/tlv.rs`

- [ ] **Step 1: Write packet.rs** — IsisPacket, IsisHeader, IihPacket, LspPacket, CsnpPacket, PsnpPacket with all wire-format fields.

- [ ] **Step 2: Write tlv.rs** — IsisTlv enum with 13 variants, parse_tlvs(&[u8]) → Vec<IsisTlv>, build_tlvs(&[IsisTlv]) → Vec<u8>.

- [ ] **Step 3: Build + commit**

---

### Task 3: Adjacency FSM + LSP DB + SPF

**Files:** Create `src/adjacency.rs`, `src/lsp.rs`, `src/spf.rs`

- [ ] **Step 1: Write adjacency.rs** — Adjacency struct, AdjacencyState enum (Down/Init/Up), process_hello(), holding_timer_expired().

- [ ] **Step 2: Write lsp.rs** — LspDatabase, LspEntry, LspId, insert/get/flood/purge_expired/generate_self_lsp.

- [ ] **Step 3: Write spf.rs** — compute_spf() pure function implementing Dijkstra.

- [ ] **Step 4: Build + commit**

---

### Task 4: Actor Implementation + Transport + Timer

**Files:** Create `src/actor.rs`, `src/transport.rs`, `src/timer.rs`

- [ ] **Step 1: Write transport.rs** — IsisTransport trait (async send/recv) + RawSocketTransport impl.

- [ ] **Step 2: Write timer.rs** — Hello interval, LSP refresh, CSNP interval management.

- [ ] **Step 3: Write actor.rs** — IsisActor with select! event loop integrating commands, packets, timers, interface events.

- [ ] **Step 4: Write lib.rs** — Re-exports, module declarations.

- [ ] **Step 5: Build + commit**

---

### Task 5: CLI + Integration

**Files:** Modify `netpilotd/src/cli.rs`

- [ ] **Step 1: Add CLI commands** — show isis topology, show isis adjacencies, show isis database.

- [ ] **Step 2: Build + commit**

---

### Task 6: Tests

**Files:** Create `tests/adjacency_tests.rs`, `tests/spf_tests.rs`, `tests/tlv_tests.rs`

- [ ] **Step 1: Adjacency FSM tests** — Down→Init, Init→Up, Up→Down transitions.

- [ ] **Step 2: SPF tests** — Single node, triangle topology, two-level hierarchy.

- [ ] **Step 3: TLV round-trip tests** — Each TLV type parses correctly.

- [ ] **Step 4: Run full test suite + commit**

---

### Final Verification

```bash
cargo fmt --check
cargo test
```

Expected: ~200+ tests, 0 failures.
