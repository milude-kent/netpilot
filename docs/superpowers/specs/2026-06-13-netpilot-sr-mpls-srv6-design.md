# NetPilot M5 — SR-MPLS & SRv6 Configuration Design

Date: 2026-06-13

## Goal

Implement Segment Routing configuration and seed runtime for M5 Tasks 3+4: SR-MPLS (#317) and SRv6 (#318). Build complete schema with validation for prefix-SID, adjacency-SID, SRGB, SRv6 SID types (End, End.X, End.T, End.DT4, End.DT6), and SRv6 locators. Add a minimal in-memory SidRegistry and a pure-function label stack builder — no IGP flooding, FIB installation, or SRH insertion (deferred to M6 when IGP protocols mature).

## Scope

### In scope

| Feature | Reference | Description |
|---------|-----------|-------------|
| SR-MPLS prefix-SID | #317 | Prefix-to-label binding: absolute or indexed SID, flags (N-flag-clear, PHP, explicit-null) |
| SR-MPLS adjacency-SID | #317 | Per-interface/neighbor SID: absolute or dynamic, protected flag |
| SRGB configuration | #317 seed | sr_global_block on MplsDomain validated against domain label ranges |
| SRv6 locators | #318 | Prefix, block/node/function length split (seeded in M5 Task 1; now validated) |
| SRv6 SID types | #318 | End, End.X, End.T, End.DT4, End.DT6 — each with behavior-specific fields |
| SidRegistry runtime | SR runtime | In-memory registry: resolve prefix-SID by prefix, list all entries |
| compute_label_stack | SR runtime | Pure function: longest-prefix-match on destination, returns single-label stack |
| SR validation rules | config | SRGB in range, SID bounds, locator/SID references, function length |
| CLI stubs | CLI | `show sr prefix-sids`, `show srv6 sids` |

### Out of scope (M6+)

- IGP flooding (OSPF SR extensions, IS-IS SR)
- MPLS label stack installation into kernel FIB
- SRv6 SRH insertion (Linux seg6 operations)
- SR-TE policies and explicit paths
- BGP SR policy / BGP-LS SR extensions

## Architecture

```
RoutePlaneConfig
  + sr_prefix_sids: Option<Vec<SrPrefixSidConfig>>       ← new
  + sr_adjacency_sids: Option<Vec<SrAdjacencySidConfig>> ← new
  + srv6_sids: Option<Vec<Srv6SidConfig>>                ← new
  + srv6_locators: Option<Vec<Srv6LocatorConfig>>        ← existing seed, now validated

MplsDomain
  + sr_enabled: Option<bool>          ← existing seed, now validated
  + sr_global_block: Option<MplsLabelRange>  ← existing seed, now validated
```

SR-MPLS and SRv6 are independent subsystems — they share a conceptual layer (Segment Routing) but operate on different forwarding planes (MPLS labels vs IPv6 headers). No cross-references between them.

Runtime:
- `SidRegistry` holds prefix-SID and adjacency-SID entries in memory, built from config at startup
- `compute_label_stack` is a pure function taking a registry reference and destination prefix, returning `Option<Vec<u32>>`
- No IGP interaction in this phase; clear API boundaries for M6 integration

## Data Structures

All new config types in `crates/netpilot-config/src/schema.rs`.

### SrPrefixSidConfig

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SrPrefixSidConfig {
    pub prefix: String,
    pub domain: String,
    pub sid_type: SrSidType,
    pub flags: SrPrefixSidFlags,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SrSidType {
    Absolute(u32),
    Index(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SrPrefixSidFlags {
    pub n_flag_clear: Option<bool>,
    pub php: Option<bool>,
    pub explicit_null: Option<bool>,
}
```

Fields:
- `prefix`: destination prefix this SID represents
- `domain`: references MplsDomain.name — the label space for this SID
- `sid_type`: `Absolute(label)` uses the label directly; `Index(n)` means `SRGB_start + n`
- `flags.n_flag_clear`: clear the N-flag (Node flag) — default false
- `flags.php`: Penultimate Hop Popping — default false
- `flags.explicit_null`: use explicit-null label — default false

### SrAdjacencySidConfig

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SrAdjacencySidConfig {
    pub interface: String,
    pub neighbor: String,
    pub domain: String,
    pub sid_type: SrAdjSidType,
    pub protected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SrAdjSidType {
    Absolute(u32),
    Dynamic,
}
```

Fields:
- `interface`: egress interface name
- `neighbor`: next-hop IP address
- `domain`: references MplsDomain.name
- `sid_type`: `Absolute(label)` or `Dynamic` (auto-allocated from domain's LabelPool)
- `protected`: whether backup path is desired (TI-LFA seed)

### Srv6SidConfig

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "kebab-case")]
pub enum Srv6SidConfig {
    End {
        name: String,
        locator: String,
        function: u32,
    },
    EndX {
        name: String,
        locator: String,
        function: u32,
        interface: String,
        nexthop: String,
    },
    EndT {
        name: String,
        locator: String,
        function: u32,
        vrf: String,
    },
    EndDT4 {
        name: String,
        locator: String,
        function: u32,
        vrf: String,
    },
    EndDT6 {
        name: String,
        locator: String,
        function: u32,
        vrf: String,
    },
}
```

All variants share `name + locator + function`. Behavior-specific fields:
- End: no extra fields (simple endpoint)
- End.X: `interface` + `nexthop` (L3 cross-connect)
- End.T: `vrf` (specific table lookup)
- End.DT4: `vrf` (decapsulate to IPv4)
- End.DT6: `vrf` (decapsulate to IPv6)

`#[serde(tag = "behavior")]` flattens JSON: `{"behavior": "end", "name": "sid1", "locator": "loc1", "function": 1}`.

### Srv6LocatorConfig (already exists, now validated)

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

### Top-level RoutePlaneConfig additions

```rust
pub struct RoutePlaneConfig {
    // ... existing fields ...
    pub sr_prefix_sids: Option<Vec<SrPrefixSidConfig>>,
    pub sr_adjacency_sids: Option<Vec<SrAdjacencySidConfig>>,
    pub srv6_sids: Option<Vec<Srv6SidConfig>>,
    // srv6_locators already exists from Task 1
}
```

## Runtime: SidRegistry + compute_label_stack

New file: `crates/netpilotd/src/sr.rs`

### SidRegistry

```rust
use netpilot_config::RoutePlaneConfig;

#[derive(Clone, Debug, Default)]
pub struct SidRegistry {
    pub prefix_sids: Vec<SrPrefixSidEntry>,
    pub adjacency_sids: Vec<SrAdjacencySidEntry>,
}

#[derive(Clone, Debug)]
pub struct SrPrefixSidEntry {
    pub prefix: String,
    pub label: u32,
    pub domain: String,
}

#[derive(Clone, Debug)]
pub struct SrAdjacencySidEntry {
    pub interface: String,
    pub neighbor: String,
    pub label: u32,
    pub domain: String,
}

impl SidRegistry {
    pub fn from_config(config: &RoutePlaneConfig) -> Self {
        let mut registry = Self::default();
        // Load prefix-SIDs from config, resolving Index to absolute labels
        if let Some(sids) = &config.sr_prefix_sids {
            for sid in sids {
                let label = match &sid.sid_type {
                    netpilot_config::SrSidType::Absolute(l) => *l,
                    netpilot_config::SrSidType::Index(idx) => {
                        // Resolve against domain's SRGB if available, else treat index as label
                        resolve_index_in_srgb(config, &sid.domain, *idx)
                    }
                };
                registry.prefix_sids.push(SrPrefixSidEntry {
                    prefix: sid.prefix.clone(),
                    label,
                    domain: sid.domain.clone(),
                });
            }
        }
        // Load adjacency-SIDs
        if let Some(sids) = &config.sr_adjacency_sids {
            for sid in sids {
                let label = match &sid.sid_type {
                    netpilot_config::SrAdjSidType::Absolute(l) => *l,
                    netpilot_config::SrAdjSidType::Dynamic => 0, // placeholder; M6: allocate from pool
                };
                registry.adjacency_sids.push(SrAdjacencySidEntry {
                    interface: sid.interface.clone(),
                    neighbor: sid.neighbor.clone(),
                    label,
                    domain: sid.domain.clone(),
                });
            }
        }
        registry
    }

    pub fn resolve_prefix_sid(&self, prefix: &str) -> Option<u32> {
        // Longest prefix match
        self.prefix_sids
            .iter()
            .filter(|e| prefix_matches(prefix, &e.prefix))
            .max_by_key(|e| prefix_len(&e.prefix))
            .map(|e| e.label)
    }
}

fn resolve_index_in_srgb(config: &RoutePlaneConfig, domain: &str, index: u32) -> u32 {
    if let Some(domains) = &config.mpls_domains {
        if let Some(d) = domains.iter().find(|d| d.name == domain) {
            if let Some(ref srgb) = d.sr_global_block {
                return srgb.low + index;
            }
        }
    }
    index // fallback: treat index as label
}

fn prefix_matches(addr: &str, prefix: &str) -> bool {
    // Simplified: use string comparison for schema-level testing
    // Full prefix matching requires a prefix library (M6)
    addr.starts_with(prefix.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ':' || c == '/'))
        || addr == prefix
}

fn prefix_len(prefix: &str) -> usize {
    prefix
        .split('/')
        .nth(1)
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(0)
}
```

### compute_label_stack

```rust
pub fn compute_label_stack(
    registry: &SidRegistry,
    destination: &str,
) -> Option<Vec<u32>> {
    registry.resolve_prefix_sid(destination).map(|label| vec![label])
    // Future (M6): prepend adjacency-SIDs for the path
}
```

## Validation Rules

Added to `crates/netpilot-config/src/validation.rs`:

1. **SRGB in domain range:** `sr_global_block` must be fully contained within at least one of the domain's `label_ranges`
2. **SR enabled requires SRGB:** if `sr_enabled == true`, `sr_global_block` must be present
3. **Prefix-SID domain reference:** `SrPrefixSidConfig.domain` must reference an existing MplsDomain
4. **Adjacency-SID domain reference:** `SrAdjacencySidConfig.domain` must reference an existing MplsDomain
5. **Absolute SID in SRGB:** absolute prefix-SID labels must fall within the referenced domain's SRGB
6. **Index SID bounds:** index values must not overflow the SRGB (`SRGB_start + index <= SRGB_end`)
7. **Srv6 SID function bounds:** `function` value must fit within the locator's `function_len` bits (`function < 2^function_len`)
8. **Srv6 SID locator reference:** `Srv6SidConfig.locator` must reference an existing `Srv6LocatorConfig.name`
9. **Srv6 locator prefix validity:** locator prefix must parse as a valid IPv6 prefix
10. **Srv6 locator length sum:** `block_len + node_len + function_len` must not exceed 128

## Files Changed

| File | Change | Approx lines |
|------|--------|-------------|
| `crates/netpilot-config/src/schema.rs` | 6 new types + 3 new fields on RoutePlaneConfig | +130 |
| `crates/netpilot-config/src/validation.rs` | 10 SR validation rules | +100 |
| `crates/netpilot-config/src/lib.rs` | Re-export new types | +5 |
| `crates/netpilotd/src/sr.rs` | SidRegistry + compute_label_stack + 8 unit tests | +180 |
| `crates/netpilotd/src/lib.rs` | Add `pub mod sr` | +1 |
| `crates/netpilotd/src/cli.rs` | `show sr prefix-sids` + `show srv6 sids` stubs | +20 |
| `crates/netpilot-config/tests/config_store.rs` | 20 SR round-trip + validation tests | +200 |
| `crates/netpilotd/tests/sr.rs` | SidRegistry + label_stack tests | +80 |
| **Total** | | **~716** |

## Test Plan

### Schema round-trip tests
- SrPrefixSidConfig with Absolute SID round-trips
- SrPrefixSidConfig with Index SID round-trips
- SrPrefixSidFlags with all flags set round-trips
- SrAdjacencySidConfig with Absolute SID round-trips
- SrAdjacencySidConfig with Dynamic SID round-trips
- Srv6SidConfig::End round-trips
- Srv6SidConfig::EndX round-trips
- Srv6SidConfig::End.DT4 round-trips
- Full config with SR-MPLS and SRv6 round-trips

### Validation tests
- SRGB not in domain range → error
- SR enabled without SRGB → error
- Prefix-SID with non-existent domain → error
- Absolute SID outside SRGB → error
- Index SID overflow → error
- Srv6 SID function exceeds function_len → error
- Srv6 SID with non-existent locator → error
- SRv6 locator length sum > 128 → error
- Valid SR config passes validation

### Runtime tests (in netpilotd/tests/sr.rs)
- SidRegistry loads prefix-SIDs from config
- resolve_prefix_sid finds exact match
- resolve_prefix_sid returns None for unknown prefix
- compute_label_stack returns single-label stack for known prefix
- compute_label_stack returns None for unknown prefix
- Absolute SID is used directly
- Index SID resolves against SRGB
- Adjacency-SID with Dynamic sets label to 0 (placeholder)
