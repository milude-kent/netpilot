# NetPilot M5 — MPLS Foundation Design

Date: 2026-06-13

## Goal

Implement MPLS Foundation (Task 1) and Label Management (Task 2) from the M5 milestone, covering 8 features: MPLS domain (#240), MPLS channel (#241), label stack depth (#242), MPLS table (#255), and 4 label management features (label range allocation, dynamic/static label pools, FEC-to-label binding, static label bindings).

This phase does NOT implement Segment Routing (SR-MPLS #317, SRv6 #318) but includes schema seed fields to avoid breaking changes when SR is added later in M5.

## Scope

### In scope

| Feature | Reference | Description |
|---------|-----------|-------------|
| MPLS domain | #240 | Global label space definition with ranges, policy, and depth limit |
| MPLS channel | #241 | Protocol-to-MPLS-table binding with import/export limits |
| Label stack depth | #242 | Per-domain max label stack depth constraint |
| MPLS table | #255 | Dedicated table storing routes keyed by MPLS label |
| Label range allocation | MPLS mgmt | Dynamic allocation from configured ranges, with preferred-label hints |
| Dynamic/static label pools | MPLS mgmt | Runtime pool per MplsDomain; static labels reserved explicitly |
| FEC-to-label binding | MPLS mgmt | In-memory binding map per MPLS table, queryable via CLI |
| Static label bindings | MPLS mgmt | Config-time explicit prefix→label mapping |

### Out of scope (M5 later phases)

- SR-MPLS (#317): prefix-SID, adjacency-SID, SRGB, label stack computation
- SRv6 (#318): SID types (End, End.X, End.T, End.DT4, End.DT6), SRH insertion, locator runtime
- MPLS dataplane / netlink label operations (M6)
- LDP protocol (M6)

### Seed fields for future SR

- `MplsDomain.sr_enabled` (bool, parsed but not wired to runtime)
- `MplsDomain.sr_global_block` (label range for prefix-SID indexing)
- `Srv6LocatorConfig` top-level list (schema-only, no runtime)

## Architecture

```
RoutePlaneConfig
  + mpls_domains: Option<Vec<MplsDomain>>      ← new
  + mpls_tables: Option<Vec<MplsTableConfig>>   ← new
  + srv6_locators: Option<Vec<Srv6LocatorConfig>> ← new (schema-only)

ProtocolConfig::Bgp / ::Static / ::Ospf
  + mpls_channel: Option<MplsChannelConfig>     ← new
```

Rationale:
- Domains are a top-level list — they define label spaces referenced by both protocols and tables, matching BIRD2's global `mpls domain` model.
- MPLS tables are separate from regular `TableConfig` — routes are keyed by label, not prefix, so they need their own type with different selection semantics.
- Channels use an optional `mpls_channel` field on each protocol variant rather than a generic union — avoids breaking existing channel semantics.
- References use names (e.g. `table.domain` → `MplsDomain.name`) for readability and BIRD2 compatibility; validation enforces referential integrity.

## Data Structures

All new types live in `crates/netpilot-config/src/schema.rs`.

### MplsDomain

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsDomain {
    pub name: String,
    pub label_ranges: Vec<MplsLabelRange>,
    pub label_policy: Option<MplsLabelPolicy>,
    pub max_label_stack_depth: Option<u8>,
    pub sr_enabled: Option<bool>,
    pub sr_global_block: Option<MplsLabelRange>,
    pub static_bindings: Option<Vec<MplsStaticBinding>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsLabelRange {
    pub low: u32,
    pub high: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MplsLabelPolicy {
    Static,
    PerPrefix,
    Aggregate,
    Vrf,
}
```

- `name`: unique domain identifier referenced by protocols and tables
- `label_ranges`: one or more label ranges owned by this domain; BIRD2 allows multiple ranges per domain
- `label_policy`: how labels are assigned beyond static allocation (`Static` = only explicit static bindings, `PerPrefix` = one label per unique prefix, `Aggregate` = shared label for aggregates, `Vrf` = per-VRF label)
- `max_label_stack_depth`: corresponds to #242 — caps label stack depth before routes enter FIB; default 8
- `sr_enabled` / `sr_global_block`: schema seed fields for SR-MPLS (#317); parsed but not wired to runtime in this phase

### MplsTableConfig

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsTableConfig {
    pub name: String,
    pub domain: String,
    pub gc_threshold: Option<u32>,
    pub gc_period_secs: Option<u32>,
    pub sorted: Option<bool>,
    pub min_settle_time_secs: Option<u32>,
    pub max_settle_time_secs: Option<u32>,
}
```

- `domain`: references `MplsDomain.name` — the table inherits the domain's ranges and policies
- Subset of regular `TableConfig` fields; omits fields that don't apply to label-keyed tables (`kernel_table`, `nettype`, `trie`)

### MplsChannelConfig

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsChannelConfig {
    pub table: String,
    pub import_limit: Option<u32>,
    pub import_limit_action: Option<LimitAction>,
    pub export_limit: Option<u32>,
    pub export_limit_action: Option<LimitAction>,
    pub import_keep_filtered: Option<bool>,
}
```

Reuses `LimitAction` enum from existing channel limits. Added as optional field on each protocol variant:

```rust
// Inside ProtocolConfig::Static, ::Bgp, ::Ospf:
pub mpls_channel: Option<MplsChannelConfig>,
```

### MplsStaticBinding

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsStaticBinding {
    pub prefix: String,
    pub label: u32,
    pub domain: Option<String>,
}
```

Optionally embedded in `MplsDomain`. Provides the equivalent of BIRD2's `static label` — explicit prefix→label binding for protocols that want a specific label without going through dynamic allocation.

### Design decision: MplsStaticBinding lives on MplsDomain

Static bindings belong to a specific domain's label space. Placing them as a field on `MplsDomain` rather than a top-level list keeps the ownership clear: a static binding is only valid within the label ranges of its parent domain, and validation is simpler (no cross-referencing needed).

### Srv6LocatorConfig (schema-only seed)

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Srv6LocatorConfig {
    pub name: String,
    pub prefix: String,
    pub block_len: Option<u8>,
    pub node_len: Option<u8>,
    pub function_len: Option<u8>,
}
```

Added as `RoutePlaneConfig.srv6_locators: Option<Vec<Srv6LocatorConfig>>`. Schema-only — no runtime behavior or validation beyond field types in this phase.

### Top-level RoutePlaneConfig changes

```rust
pub struct RoutePlaneConfig {
    // ... existing fields ...
    pub mpls_domains: Option<Vec<MplsDomain>>,
    pub mpls_tables: Option<Vec<MplsTableConfig>>,
    pub srv6_locators: Option<Vec<Srv6LocatorConfig>>,
}
```

## Runtime: LabelPool

LabelPool lives in `crates/netpilotd/src/state.rs` (or a new `mpls.rs` module under netpilotd). It is instantiated per `MplsDomain` at daemon startup.

```rust
pub struct LabelPool {
    ranges: Vec<MplsLabelRange>,
    allocated: BTreeSet<u32>,
}

impl LabelPool {
    /// Allocate the next available label from the pool.
    /// If `preferred` is given and available, it is used; otherwise next free.
    pub fn allocate(&mut self, preferred: Option<u32>) -> Option<u32>;

    /// Reserve a static label. Returns error if label is out of range or already allocated.
    pub fn allocate_static(&mut self, label: u32) -> Result<(), LabelError>;

    /// Release a previously allocated label back to the pool.
    pub fn free(&mut self, label: u32);

    /// Check whether a label is available.
    pub fn is_available(&self, label: u32) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum LabelError {
    #[error("label {0} is outside the domain's configured ranges")]
    OutOfRange(u32),
    #[error("label {0} is already allocated")]
    AlreadyAllocated(u32),
}
```

Behavior:
- `allocate()` picks the first free label across all configured ranges (sorted low→high), or honors the `preferred` hint if available
- `allocate_static()` validates the label falls within configured ranges, then reserves it atomically
- Allocations are not persisted across restarts — matches BIRD2 behavior (re-allocate fresh after restart)
- A `HashMap<String, LabelPool>` (domain name → pool) is held in daemon state

### FecLabelBinding

In-memory binding map for querying which FEC owns which label:

```rust
#[derive(Clone, Debug)]
pub struct FecLabelBinding {
    pub prefix: String,
    pub label: u32,
    pub domain: String,
    pub source: LabelSource,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelSource {
    Static,
    Protocol { instance_name: String },
    /// Reserved for future use (SR-MPLS dynamic SID allocation, LDP).
    /// Not produced by any code path in this phase.
    Auto,
}
```

Stored as `Vec<FecLabelBinding>` per MPLS table or as a single daemon-wide registry keyed by `(domain, label)`.

Note: `LabelSource::Auto` is a seed variant for future SR-MPLS and LDP phases. No code path produces it in this phase — it exists only in the enum definition so we don't need a breaking schema change later.

## Validation Rules

Added to `crates/netpilot-config/src/validation.rs`:

1. **Domain uniqueness:** `mpls_domains[].name` must be unique
2. **Range validity:** each `MplsLabelRange` must satisfy `low >= 16` (reserved labels 0–15), `high >= low`, `high <= 1_048_575` (20-bit label space)
3. **Range overlap:** two ranges within the same domain must not overlap
4. **Static binding labels in range:** each `MplsStaticBinding.label` must fall within at least one range of the referenced domain
5. **Static binding label uniqueness:** two static bindings in the same domain must not claim the same label
6. **Table domain reference:** `MplsTableConfig.domain` must reference an existing `MplsDomain.name`
7. **Channel table reference:** `MplsChannelConfig.table` must reference an existing `MplsTableConfig.name`
8. **Stack depth:** `max_label_stack_depth` if set must be in range `1..=32`

## Files Changed

| File | Change | Approx lines |
|------|--------|-------------|
| `crates/netpilot-config/src/schema.rs` | Add 7 new types + 3 new fields on RoutePlaneConfig + mpls_channel on ProtocolConfig variants | +200 |
| `crates/netpilot-config/src/validation.rs` | Add 8 validation rules | +80 |
| `crates/netpilot-config/src/store.rs` | No code change (derives handle it); add MPLS fixtures to tests | +30 |
| `crates/netpilotd/src/state.rs` | Add LabelPool, FecLabelBinding, domain→pool map | +100 |
| `crates/netpilotd/src/mpls.rs` | New module: LabelPool implementation | +80 |
| `crates/netpilotd/src/cli.rs` | Optional: `show mpls labels` command | +30 |
| `crates/netpilot-config/tests/config_store.rs` | MPLS domain/table round-trip tests | +80 |
| `crates/netpilotd/tests/api_config.rs` | MPLS config via API tests | +60 |
| **Total** | | **~660** |

## Test Plan

### Unit tests (in netpilot-config)

- Domain with single range serializes/deserializes correctly
- Domain with multiple ranges serializes/deserializes correctly
- All four MplsLabelPolicy variants round-trip
- MplsTableConfig with all fields populated round-trips
- MplsChannelConfig embedded in BGP, Static, OSPF protocol configs round-trips
- MplsStaticBinding round-trips

### Validation tests (in netpilot-config)

- Duplicate domain names → error
- Overlapping label ranges → error
- Non-overlapping label ranges → ok
- Label below 16 → error
- Label above 1_048_575 → error
- Static binding label outside domain ranges → error
- Two static bindings claiming same label → error
- Non-existent domain reference from table → error
- Non-existent table reference from channel → error
- Valid MPLS config passes all validation

### Runtime tests (in netpilotd)

- LabelPool allocates sequentially from range
- LabelPool.allocate() with free preferred label returns it
- LabelPool.allocate_static() succeeds for free label
- LabelPool.allocate_static() fails for already-allocated label
- LabelPool.allocate_static() fails for out-of-range label
- LabelPool.free() makes label available again
- LabelPool returns None when range exhausted
- Multiple domains with separate pools do not conflict

### Integration tests (existing test files)

- Config store: commit MPLS config, rollback, verify
- API: PUT candidate config with MPLS domains and tables, verify in GET response
