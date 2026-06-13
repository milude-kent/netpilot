# NetPilot M5 — MPLS Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement MPLS domain configuration, table configuration, channel configuration, static label bindings, runtime label pool, and FEC-to-label binding — 8 features across 2 M5 tasks (#240-#242, #255 + label management).

**Architecture:** New schema types in `netpilot-config/src/schema.rs` define MPLS domains, tables, channels, static bindings, and SRv6 locator seeds. Validation rules in `validation.rs` enforce domain uniqueness, range validity, referential integrity. Runtime `LabelPool` in `netpilotd/src/mpls.rs` manages per-domain label allocation. Minimal CLI stub for `show mpls labels`.

**Tech Stack:** Rust 2024 edition, serde, thiserror, tokio. No new dependencies.

---

## File Map

| File | Role |
|------|------|
| `crates/netpilot-config/src/schema.rs` | All new MPLS config types + RoutePlaneConfig/ProtocolConfig field additions |
| `crates/netpilot-config/src/lib.rs` | Re-export new types |
| `crates/netpilot-config/src/validation.rs` | 8 MPLS validation rules |
| `crates/netpilotd/src/mpls.rs` | LabelPool, FecLabelBinding, LabelSource, LabelError — NEW file |
| `crates/netpilotd/src/state.rs` | Add MplsState: HashMap<String, LabelPool>, Vec<FecLabelBinding> |
| `crates/netpilotd/src/lib.rs` | Declare new mpls module |
| `crates/netpilotd/src/cli.rs` | Add `ShowMplsLabels` variant + parse + execute stub |
| `crates/netpilot-config/tests/config_store.rs` | MPLS config round-trip + commit/rollback tests |
| `crates/netpilotd/tests/mpls.rs` | LabelPool unit tests — NEW file |
| `crates/netpilotd/tests/api_config.rs` | MPLS config via API test |

---

### Task 1: MPLS Schema Types

**Files:**
- Modify: `crates/netpilot-config/src/schema.rs`

**Purpose:** Add all 7 new MPLS types and wire them into `RoutePlaneConfig` and `ProtocolConfig` variants.

- [ ] **Step 1: Add MPLS types to schema.rs**

Append the following types at the end of `crates/netpilot-config/src/schema.rs`, after the existing `CliSocketConfig` definition (line 305) and before the `impl From<NettypeDef> for Nettype` block (line 307):

```rust
// ── MPLS Domain ──────────────────────────────────────────────

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

// ── MPLS Static Binding ──────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsStaticBinding {
    pub prefix: String,
    pub label: u32,
}

// ── MPLS Table ───────────────────────────────────────────────

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

// ── MPLS Channel ─────────────────────────────────────────────

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

// ── SRv6 Locator (schema-only seed) ──────────────────────────

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

- [ ] **Step 2: Add new fields to RoutePlaneConfig**

Modify the `RoutePlaneConfig` struct (lines 4-26). Replace the closing `}` of the struct (line 26) to add the three new fields before it:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RoutePlaneConfig {
    pub schema_version: u32,
    pub identity: RouterIdentity,
    pub tables: Vec<TableConfig>,
    pub protocols: Vec<ProtocolConfig>,
    pub hostname: Option<String>,
    pub defines: Option<Vec<ConstantDef>>,
    pub cli_sockets: Option<Vec<CliSocketConfig>>,
    pub watchdog_warning_secs: Option<u32>,
    pub watchdog_timeout_secs: Option<u32>,
    pub debug_latency: Option<bool>,
    pub debug_latency_limit_micros: Option<u64>,
    pub debug_protocols: Option<String>,
    pub debug_channels: Option<String>,
    pub debug_tables: Option<String>,
    pub debug_commands: Option<u8>,
    pub timeformat_route: Option<String>,
    pub timeformat_protocol: Option<String>,
    pub timeformat_base: Option<String>,
    pub timeformat_log: Option<String>,
    pub mpls_domains: Option<Vec<MplsDomain>>,
    pub mpls_tables: Option<Vec<MplsTableConfig>>,
    pub srv6_locators: Option<Vec<Srv6LocatorConfig>>,
}
```

Update the `Default` impl (lines 28-62) by adding the three new fields with `None` before the closing `}`:

```rust
            timeformat_route: None,
            timeformat_protocol: None,
            timeformat_base: None,
            timeformat_log: None,
            mpls_domains: None,
            mpls_tables: None,
            srv6_locators: None,
```

- [ ] **Step 3: Add mpls_channel to ProtocolConfig variants**

Add `mpls_channel: Option<MplsChannelConfig>` to each of the three ProtocolConfig variants.

For `ProtocolConfig::Static` (lines 89-101):

```rust
    Static {
        name: String,
        table: String,
        routes: Vec<StaticRoute>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
```

For `ProtocolConfig::Bgp` (lines 102-126):

```rust
    Bgp {
        name: String,
        table: String,
        local_asn: u32,
        neighbors: Vec<BgpNeighbor>,
        import_table: Option<String>,
        export_table: Option<String>,
        update_delay_secs: Option<u32>,
        advertisement_delay_secs: Option<u32>,
        coalesce_time_millis: Option<u32>,
        listen_range: Option<String>,
        vrf: Option<String>,
        view: Option<String>,
        from_template: Option<String>,
        aspa_downstream_check: Option<bool>,
        aspa_upstream_check: Option<bool>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
```

For `ProtocolConfig::Ospf` (lines 127-147):

```rust
    Ospf {
        name: String,
        table: String,
        router_id: Option<String>,
        instance_id: Option<u8>,
        ecmp: Option<bool>,
        ecmp_limit: Option<u32>,
        areas: Vec<OspfAreaConfig>,
        stub_router: Option<bool>,
        rfc1583_compat: Option<bool>,
        merge_external: Option<bool>,
        tick_secs: Option<u32>,
        from_template: Option<String>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
```

- [ ] **Step 4: Build check**

Run: `cargo build -p netpilot-config 2>&1`
Expected: PASS (compiles successfully, no warnings)

- [ ] **Step 5: Commit**

```bash
git add crates/netpilot-config/src/schema.rs
git commit -m "feat: add MPLS domain, table, channel, static binding, and SRv6 locator schema types (#240-#242, #255 seed)

Adds MplsDomain, MplsLabelRange, MplsLabelPolicy, MplsTableConfig,
MplsChannelConfig, MplsStaticBinding, and Srv6LocatorConfig types.
Wires mpls_domains, mpls_tables, and srv6_locators into RoutePlaneConfig.
Adds optional mpls_channel to Static, Bgp, and Ospf ProtocolConfig variants.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Update lib.rs Re-exports

**Files:**
- Modify: `crates/netpilot-config/src/lib.rs`

- [ ] **Step 1: Re-export new MPLS types**

Replace the `pub use schema::{...}` block in `crates/netpilot-config/src/lib.rs` (lines 6-10):

```rust
pub use schema::{
    AddressFamily, AuthAlgorithm, AuthPassword, BgpNeighbor, ChannelLimits, CliSocketConfig,
    ConstantDef, GrMode, LimitAction, LinkBandwidth, MplsChannelConfig, MplsDomain,
    MplsLabelPolicy, MplsLabelRange, MplsStaticBinding, MplsTableConfig, NettypeDef,
    OspfAreaConfig, ProtocolConfig, RoutePlaneConfig, RouterIdentity, Srv6LocatorConfig,
    StaticNexthopType, StaticRoute, TableConfig, TemplateRef,
};
```

- [ ] **Step 2: Build check**

Run: `cargo build -p netpilot-config 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/netpilot-config/src/lib.rs
git commit -m "feat: re-export MPLS schema types from netpilot-config

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: MPLS Validation Rules

**Files:**
- Modify: `crates/netpilot-config/src/validation.rs`

- [ ] **Step 1: Add MPLS validation logic**

Append the following validation function and integrate it into `validate_config`. First, add a helper function for MPLS validation after the existing `validate_config` function (after line 80):

```rust
fn validate_mpls(config: &RoutePlaneConfig) -> Result<Vec<String>, ValidationError> {
    let mut warnings = Vec::new();

    let domains = match &config.mpls_domains {
        Some(d) => d,
        None => return Ok(warnings),
    };

    // 1. Domain name uniqueness
    let mut domain_names = HashSet::new();
    for domain in domains {
        if !domain_names.insert(&domain.name) {
            return Err(ValidationError::Message(format!(
                "duplicate MPLS domain name '{}'",
                domain.name
            )));
        }
    }

    // 2. Range validity and 3. overlap check
    for domain in domains {
        let mut ranges_sorted: Vec<&MplsLabelRange> = domain.label_ranges.iter().collect();
        ranges_sorted.sort_by_key(|r| r.low);

        for range in &domain.label_ranges {
            if range.low < 16 {
                return Err(ValidationError::Message(format!(
                    "MPLS domain '{}': label range low {} is below reserved range (0-15)",
                    domain.name, range.low
                )));
            }
            if range.high > 1_048_575 {
                return Err(ValidationError::Message(format!(
                    "MPLS domain '{}': label range high {} exceeds 20-bit label space (1_048_575)",
                    domain.name, range.high
                )));
            }
            if range.low > range.high {
                return Err(ValidationError::Message(format!(
                    "MPLS domain '{}': label range low {} > high {}",
                    domain.name, range.low, range.high
                )));
            }
        }

        // Overlap check
        for window in ranges_sorted.windows(2) {
            let (a, b) = (window[0], window[1]);
            if a.high >= b.low {
                return Err(ValidationError::Message(format!(
                    "MPLS domain '{}': label ranges [{}, {}] and [{}, {}] overlap",
                    domain.name, a.low, a.high, b.low, b.high
                )));
            }
        }

        // 4. Static binding labels in range + 5. uniqueness
        if let Some(bindings) = &domain.static_bindings {
            let mut binding_labels = HashSet::new();
            for binding in bindings {
                let in_range = domain
                    .label_ranges
                    .iter()
                    .any(|r| binding.label >= r.low && binding.label <= r.high);
                if !in_range {
                    return Err(ValidationError::Message(format!(
                        "MPLS domain '{}': static binding label {} for prefix '{}' is outside configured ranges",
                        domain.name, binding.label, binding.prefix
                    )));
                }
                if !binding_labels.insert(binding.label) {
                    return Err(ValidationError::Message(format!(
                        "MPLS domain '{}': duplicate static binding label {}",
                        domain.name, binding.label
                    )));
                }
            }
        }

        // 8. Stack depth range
        if let Some(depth) = domain.max_label_stack_depth {
            if depth < 1 || depth > 32 {
                return Err(ValidationError::Message(format!(
                    "MPLS domain '{}': max_label_stack_depth {} out of range [1, 32]",
                    domain.name, depth
                )));
            }
        }
    }

    // 6. MPLS table domain references
    if let Some(tables) = &config.mpls_tables {
        for table in tables {
            if !domain_names.contains(&table.domain) {
                return Err(ValidationError::Message(format!(
                    "MPLS table '{}' references non-existent domain '{}'",
                    table.name, table.domain
                )));
            }
        }
    }

    // 7. MPLS channel table references
    if let Some(mpls_tables) = &config.mpls_tables {
        let table_names: HashSet<&str> = mpls_tables.iter().map(|t| t.name.as_str()).collect();

        for protocol in &config.protocols {
            let mpls_channel = match protocol {
                ProtocolConfig::Static { mpls_channel, .. } => mpls_channel,
                ProtocolConfig::Bgp { mpls_channel, .. } => mpls_channel,
                ProtocolConfig::Ospf { mpls_channel, .. } => mpls_channel,
            };
            if let Some(channel) = mpls_channel {
                if !table_names.contains(channel.table.as_str()) {
                    return Err(ValidationError::Message(format!(
                        "MPLS channel references non-existent MPLS table '{}'",
                        channel.table
                    )));
                }
            }
        }
    }

    Ok(warnings)
}
```

- [ ] **Step 2: Wire validate_mpls into validate_config**

In `validate_config` (line 16), add the MPLS validation call before the final `Ok(ValidationReport { warnings })` return. After the existing protocol validation loop (after line 77, before line 79):

```rust
    // MPLS validation
    let mpls_warnings = validate_mpls(config)?;
    warnings.extend(mpls_warnings);

    Ok(ValidationReport { warnings })
```

The entire end of `validate_config` should now look like:

```rust
            }
        }

        // Check MPLS channel table references
        if let ProtocolConfig::Static { mpls_channel, .. } = protocol {
            if let Some(ref ch) = mpls_channel {
                // Validation is done in validate_mpls
            }
        }
    }

    // MPLS validation
    let mpls_warnings = validate_mpls(config)?;
    warnings.extend(mpls_warnings);

    Ok(ValidationReport { warnings })
}
```

Note: Remove the dead match arm code — we just need the `validate_mpls` call. The cleanest approach: replace the entire existing `validate_config` body ending. The exact text to replace (lines 76-80 of the original file):

```rust
        }
    }

    Ok(ValidationReport { warnings })
}
```

Replace with:

```rust
        }
    }

    // MPLS validation
    let mpls_warnings = validate_mpls(config)?;
    warnings.extend(mpls_warnings);

    Ok(ValidationReport { warnings })
}
```

And add the needed import for `MplsLabelRange` at the top:

```rust
use crate::schema::{MplsLabelRange, ProtocolConfig, RoutePlaneConfig, StaticNexthopType};
```

- [ ] **Step 3: Build + test check**

Run: `cargo build -p netpilot-config 2>&1 && cargo test -p netpilot-config 2>&1`
Expected: PASS (existing tests still pass)

- [ ] **Step 4: Commit**

```bash
git add crates/netpilot-config/src/validation.rs
git commit -m "feat: add MPLS validation rules — domain uniqueness, range validity, references (#240-#255)

8 validation rules: domain name uniqueness, label range bounds (16-1048575),
range overlap detection, static binding label in-range, static binding label
uniqueness, MPLS table domain reference, MPLS channel table reference, stack
depth range (1-32).

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Schema Round-trip Tests

**Files:**
- Modify: `crates/netpilot-config/tests/config_store.rs`

- [ ] **Step 1: Add MPLS schema round-trip and validation tests**

Append the following tests at the end of `crates/netpilot-config/tests/config_store.rs`:

```rust
// ── MPLS Schema Round-trip Tests ─────────────────────────────

use netpilot_config::{
    MplsChannelConfig, MplsDomain, MplsLabelPolicy, MplsLabelRange, MplsStaticBinding,
    MplsTableConfig,
};

#[test]
fn mpls_domain_round_trips_as_json() {
    let domain = MplsDomain {
        name: "main".into(),
        label_ranges: vec![MplsLabelRange {
            low: 100,
            high: 200,
        }],
        label_policy: Some(MplsLabelPolicy::PerPrefix),
        max_label_stack_depth: Some(8),
        sr_enabled: None,
        sr_global_block: None,
        static_bindings: None,
    };

    let encoded = serde_json::to_string(&domain).expect("serializes");
    let decoded: MplsDomain = serde_json::from_str(&encoded).expect("deserializes");

    assert_eq!(decoded.name, "main");
    assert_eq!(decoded.label_ranges.len(), 1);
    assert_eq!(decoded.label_ranges[0].low, 100);
    assert_eq!(decoded.label_ranges[0].high, 200);
    assert!(matches!(decoded.label_policy, Some(MplsLabelPolicy::PerPrefix)));
    assert_eq!(decoded.max_label_stack_depth, Some(8));
}

#[test]
fn mpls_domain_with_multiple_ranges_round_trips() {
    let domain = MplsDomain {
        name: "dual".into(),
        label_ranges: vec![
            MplsLabelRange { low: 100, high: 199 },
            MplsLabelRange { low: 300, high: 399 },
        ],
        label_policy: None,
        max_label_stack_depth: None,
        sr_enabled: None,
        sr_global_block: None,
        static_bindings: Some(vec![MplsStaticBinding {
            prefix: "10.0.0.0/8".into(),
            label: 150,
        }]),
    };

    let encoded = serde_json::to_string(&domain).expect("serializes");
    let decoded: MplsDomain = serde_json::from_str(&encoded).expect("deserializes");

    assert_eq!(decoded.label_ranges.len(), 2);
    assert_eq!(
        decoded.static_bindings.as_ref().unwrap()[0].label,
        150
    );
    assert_eq!(
        decoded.static_bindings.as_ref().unwrap()[0].prefix,
        "10.0.0.0/8"
    );
}

#[test]
fn all_four_mpls_label_policy_variants_round_trip() {
    for (policy, expected_key) in [
        (MplsLabelPolicy::Static, "static"),
        (MplsLabelPolicy::PerPrefix, "per-prefix"),
        (MplsLabelPolicy::Aggregate, "aggregate"),
        (MplsLabelPolicy::Vrf, "vrf"),
    ] {
        let encoded = serde_json::to_string(&policy).expect("serializes");
        assert!(
            encoded.contains(expected_key),
            "expected '{}' in '{}'",
            expected_key,
            encoded
        );
        let _decoded: MplsLabelPolicy =
            serde_json::from_str(&encoded).expect("deserializes");
    }
}

#[test]
fn mpls_table_config_round_trips() {
    let table = MplsTableConfig {
        name: "mpls1".into(),
        domain: "main".into(),
        gc_threshold: Some(1000),
        gc_period_secs: Some(300),
        sorted: Some(true),
        min_settle_time_secs: Some(1),
        max_settle_time_secs: Some(10),
    };

    let encoded = serde_json::to_string(&table).expect("serializes");
    let decoded: MplsTableConfig = serde_json::from_str(&encoded).expect("deserializes");

    assert_eq!(decoded.name, "mpls1");
    assert_eq!(decoded.domain, "main");
    assert_eq!(decoded.gc_threshold, Some(1000));
    assert_eq!(decoded.sorted, Some(true));
}

#[test]
fn full_config_with_mpls_domain_and_table_round_trips() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 16,
                high: 1023,
            }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls-table-1".into(),
            domain: "main".into(),
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let encoded = serde_json::to_string(&config).expect("serializes");
    let decoded: RoutePlaneConfig = serde_json::from_str(&encoded).expect("deserializes");

    let domains = decoded.mpls_domains.expect("mpls_domains present");
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name, "main");
    let tables = decoded.mpls_tables.expect("mpls_tables present");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "mpls-table-1");
}

// ── MPLS Validation Tests ────────────────────────────────────

#[test]
fn validation_rejects_duplicate_mpls_domain_names() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![
            MplsDomain {
                name: "dup".into(),
                label_ranges: vec![MplsLabelRange { low: 16, high: 99 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: None,
                sr_global_block: None,
                static_bindings: None,
            },
            MplsDomain {
                name: "dup".into(),
                label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: None,
                sr_global_block: None,
                static_bindings: None,
            },
        ]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("duplicate domain names should fail");
    assert!(err.to_string().contains("duplicate"));
    assert!(err.to_string().contains("dup"));
}

#[test]
fn validation_rejects_overlapping_label_ranges() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![
                MplsLabelRange { low: 100, high: 200 },
                MplsLabelRange { low: 150, high: 300 },
            ],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("overlapping ranges should fail");
    assert!(err.to_string().contains("overlap"));
}

#[test]
fn validation_accepts_non_overlapping_label_ranges() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![
                MplsLabelRange { low: 100, high: 199 },
                MplsLabelRange { low: 300, high: 399 },
            ],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let report = validate_config(&config).expect("non-overlapping ranges should pass");
    assert!(report.warnings.is_empty() || report.warnings.iter().all(|w| w.contains("router-id")));
}

#[test]
fn validation_rejects_label_below_reserved_range() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 5, high: 100 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("label below 16 should fail");
    assert!(err.to_string().contains("below reserved"));
}

#[test]
fn validation_rejects_label_above_20bit_space() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 1_000_000,
                high: 2_000_000,
            }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("label above 20-bit should fail");
    assert!(err.to_string().contains("exceeds"));
}

#[test]
fn validation_rejects_static_binding_outside_ranges() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: Some(vec![MplsStaticBinding {
                prefix: "10.0.0.0/8".into(),
                label: 999,
            }]),
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("static binding outside ranges should fail");
    assert!(err.to_string().contains("outside configured ranges"));
}

#[test]
fn validation_rejects_duplicate_static_binding_labels() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 299 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: Some(vec![
                MplsStaticBinding {
                    prefix: "10.0.0.0/8".into(),
                    label: 150,
                },
                MplsStaticBinding {
                    prefix: "172.16.0.0/12".into(),
                    label: 150,
                },
            ]),
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("duplicate static binding labels should fail");
    assert!(err.to_string().contains("duplicate static binding"));
}

#[test]
fn validation_rejects_mpls_table_with_non_existent_domain() {
    let config = RoutePlaneConfig {
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls1".into(),
            domain: "ghost".into(),
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("non-existent domain reference should fail");
    assert!(err.to_string().contains("non-existent domain"));
}

#[test]
fn validation_rejects_mpls_channel_with_non_existent_table() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16, high: 99 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        protocols: vec![ProtocolConfig::Static {
            name: "stat".into(),
            table: "master".into(),
            routes: vec![],
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
            mpls_channel: Some(MplsChannelConfig {
                table: "ghost-mpls".into(),
                import_limit: None,
                import_limit_action: None,
                export_limit: None,
                export_limit_action: None,
                import_keep_filtered: None,
            }),
        }],
        ..RoutePlaneConfig::default()
    };

    let err =
        validate_config(&config).expect_err("non-existent MPLS table reference should fail");
    assert!(err.to_string().contains("non-existent MPLS table"));
}

#[test]
fn validation_rejects_invalid_max_label_stack_depth() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16, high: 99 }],
            label_policy: None,
            max_label_stack_depth: Some(0),
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let err = validate_config(&config).expect_err("stack depth 0 should fail");
    assert!(err.to_string().contains("max_label_stack_depth"));
}

#[test]
fn validation_accepts_valid_mpls_config() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
            label_policy: Some(MplsLabelPolicy::PerPrefix),
            max_label_stack_depth: Some(8),
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: Some(vec![MplsStaticBinding {
                prefix: "10.0.0.0/8".into(),
                label: 150,
            }]),
        }]),
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls1".into(),
            domain: "main".into(),
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    let report = validate_config(&config).expect("valid MPLS config should pass");
    // Only router-id warning is expected (from default)
    assert!(report.warnings.is_empty() || report.warnings.iter().all(|w| w.contains("router-id")));
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p netpilot-config 2>&1`
Expected: All tests PASS including the 15 new MPLS tests

- [ ] **Step 3: Commit**

```bash
git add crates/netpilot-config/tests/config_store.rs
git commit -m "test: add MPLS schema round-trip and validation tests (15 tests)

Tests cover: domain round-trip, multi-range, all label policy variants,
MPLS table config round-trip, full config with MPLS, and 10 validation
scenario tests.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: LabelPool Runtime Module

**Files:**
- Create: `crates/netpilotd/src/mpls.rs`

- [ ] **Step 0: Add time dependency to netpilotd**

In `crates/netpilotd/Cargo.toml`, add `time` to the dependencies:

```toml
[dependencies]
axum.workspace = true
netpilot-config = { path = "../netpilot-config" }
serde.workspace = true
serde_json.workspace = true
time.workspace = true
tokio.workspace = true
```

- [ ] **Step 1: Create the mpls module with LabelPool and FecLabelBinding**

Write the full content of `crates/netpilotd/src/mpls.rs`:

```rust
use netpilot_config::MplsLabelRange;
use std::collections::{BTreeSet, HashMap};
use time::OffsetDateTime;

/// Errors that can occur during label allocation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    #[error("label {0} is outside the domain's configured ranges")]
    OutOfRange(u32),
    #[error("label {0} is already allocated")]
    AlreadyAllocated(u32),
}

/// Source of a label assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelSource {
    /// Explicitly configured by the operator (static label binding).
    Static,
    /// Assigned to a specific protocol instance.
    Protocol { instance_name: String },
    /// Reserved for future use (SR-MPLS dynamic SID allocation, LDP).
    /// Not produced by any code path in this phase.
    #[allow(dead_code)]
    Auto,
}

/// A binding between a FEC (prefix) and an allocated label.
#[derive(Clone, Debug)]
pub struct FecLabelBinding {
    pub prefix: String,
    pub label: u32,
    pub domain: String,
    pub source: LabelSource,
    pub created_at: OffsetDateTime,
}

/// Per-domain label allocation pool.
///
/// Tracks label ranges and allocated labels, providing atomic allocate/free
/// operations. Allocations are not persisted across restarts (matching BIRD2
/// behavior).
#[derive(Clone, Debug)]
pub struct LabelPool {
    ranges: Vec<MplsLabelRange>,
    allocated: BTreeSet<u32>,
}

impl LabelPool {
    /// Create a new pool from the configured label ranges.
    /// Ranges are sorted by `low` on construction.
    pub fn new(ranges: Vec<MplsLabelRange>) -> Self {
        let mut ranges = ranges;
        ranges.sort_by_key(|r| r.low);
        Self {
            ranges,
            allocated: BTreeSet::new(),
        }
    }

    /// Allocate the next available label.
    ///
    /// If `preferred` is `Some(label)` and that label is available AND within
    /// the configured ranges, it is allocated. Otherwise the first free label
    /// is returned. Returns `None` when all labels are exhausted.
    pub fn allocate(&mut self, preferred: Option<u32>) -> Option<u32> {
        if let Some(label) = preferred {
            if self.is_in_range(label) && !self.allocated.contains(&label) {
                self.allocated.insert(label);
                return Some(label);
            }
        }

        for range in &self.ranges {
            for label in range.low..=range.high {
                if !self.allocated.contains(&label) {
                    self.allocated.insert(label);
                    return Some(label);
                }
            }
        }
        None
    }

    /// Reserve a specific static label.
    ///
    /// Returns `Ok(())` on success, or `LabelError` if the label is
    /// out of range or already allocated.
    pub fn allocate_static(&mut self, label: u32) -> Result<(), LabelError> {
        if !self.is_in_range(label) {
            return Err(LabelError::OutOfRange(label));
        }
        if self.allocated.contains(&label) {
            return Err(LabelError::AlreadyAllocated(label));
        }
        self.allocated.insert(label);
        Ok(())
    }

    /// Release a previously allocated label.
    /// No-op if the label was not allocated.
    pub fn free(&mut self, label: u32) {
        self.allocated.remove(&label);
    }

    /// Check whether a label is available (free and in range).
    pub fn is_available(&self, label: u32) -> bool {
        self.is_in_range(label) && !self.allocated.contains(&label)
    }

    /// Return the total number of labels across all ranges.
    pub fn capacity(&self) -> u64 {
        self.ranges
            .iter()
            .map(|r| (r.high - r.low + 1) as u64)
            .sum()
    }

    /// Return the count of currently allocated labels.
    pub fn allocated_count(&self) -> usize {
        self.allocated.len()
    }

    fn is_in_range(&self, label: u32) -> bool {
        self.ranges
            .iter()
            .any(|r| label >= r.low && label <= r.high)
    }
}

/// Collection of label pools keyed by domain name.
#[derive(Clone, Debug, Default)]
pub struct MplsLabelState {
    pub pools: HashMap<String, LabelPool>,
    pub bindings: Vec<FecLabelBinding>,
}

impl MplsLabelState {
    /// Initialize pools from configured MPLS domains.
    pub fn from_domains(domains: &[netpilot_config::MplsDomain]) -> Self {
        let mut state = Self::default();
        for domain in domains {
            state
                .pools
                .insert(domain.name.clone(), LabelPool::new(domain.label_ranges.clone()));
        }
        state
    }

    /// Bind a FEC to a label, recording the binding for CLI queries.
    pub fn bind(
        &mut self,
        domain: &str,
        prefix: &str,
        label: u32,
        source: LabelSource,
    ) {
        self.bindings.push(FecLabelBinding {
            prefix: prefix.to_string(),
            label,
            domain: domain.to_string(),
            source,
            created_at: OffsetDateTime::now_utc(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_allocates_sequentially() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 20 }]);
        assert_eq!(pool.allocate(None), Some(16));
        assert_eq!(pool.allocate(None), Some(17));
        assert_eq!(pool.allocate(None), Some(18));
        assert_eq!(pool.allocated_count(), 3);
    }

    #[test]
    fn pool_allocate_preferred_label() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 30 }]);
        assert_eq!(pool.allocate(Some(25)), Some(25));
        assert_eq!(pool.allocated_count(), 1);
    }

    #[test]
    fn pool_allocate_ignores_occupied_preferred_label() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 20 }]);
        pool.allocate(Some(17)); // takes 17
        // 17 is taken, should get the next free label
        let got = pool.allocate(Some(17));
        assert!(got.is_some());
        assert_ne!(got, Some(17));
    }

    #[test]
    fn pool_allocate_static_succeeds_for_free_label() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 100, high: 199 }]);
        assert!(pool.allocate_static(150).is_ok());
        assert!(!pool.is_available(150));
    }

    #[test]
    fn pool_allocate_static_fails_for_already_allocated() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 100, high: 199 }]);
        pool.allocate_static(150).unwrap();
        let err = pool.allocate_static(150).expect_err("duplicate allocation should fail");
        assert!(matches!(err, LabelError::AlreadyAllocated(150)));
    }

    #[test]
    fn pool_allocate_static_fails_for_out_of_range() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 100, high: 199 }]);
        let err = pool.allocate_static(999).expect_err("out-of-range should fail");
        assert!(matches!(err, LabelError::OutOfRange(999)));
    }

    #[test]
    fn pool_free_makes_label_available() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 19 }]);
        pool.allocate_static(17).unwrap();
        assert!(!pool.is_available(17));
        pool.free(17);
        assert!(pool.is_available(17));
    }

    #[test]
    fn pool_returns_none_when_exhausted() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 16, high: 17 }]);
        assert!(pool.allocate(None).is_some());
        assert!(pool.allocate(None).is_some());
        assert_eq!(pool.allocate(None), None);
    }

    #[test]
    fn pool_capacity_is_sum_of_range_sizes() {
        let pool = LabelPool::new(vec![
            MplsLabelRange { low: 16, high: 25 },   // 10 labels
            MplsLabelRange { low: 100, high: 109 }, // 10 labels
        ]);
        assert_eq!(pool.capacity(), 20);
    }

    #[test]
    fn pool_frees_label_correctly_from_middle_of_range() {
        let mut pool = LabelPool::new(vec![MplsLabelRange { low: 100, high: 105 }]);
        pool.allocate_static(100).unwrap();
        pool.allocate_static(101).unwrap();
        pool.allocate_static(102).unwrap();
        pool.free(101);
        // 101 is now free, 100 and 102 remain allocated
        assert!(pool.is_available(101));
        assert!(!pool.is_available(100));
        assert!(!pool.is_available(102));
    }

    #[test]
    fn mpls_label_state_initializes_pools_from_domains() {
        use netpilot_config::{MplsDomain, MplsLabelRange};

        let domains = vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }];

        let state = MplsLabelState::from_domains(&domains);
        assert!(state.pools.contains_key("main"));
        assert_eq!(state.pools.get("main").unwrap().capacity(), 100);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p netpilotd --lib 2>&1`
Expected: 10 tests PASS in mpls module

- [ ] **Step 3: Commit**

```bash
git add crates/netpilotd/src/mpls.rs
git commit -m "feat: add LabelPool, FecLabelBinding, and MplsLabelState runtime (#240, #255)

LabelPool provides per-domain label allocation with preferred label hints,
static label reservation, and label release. FecLabelBinding records FEC-to-label
mappings. MplsLabelState initializes pools from configured domains. 10 unit tests.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Wire MPLS into AppState and lib.rs

**Files:**
- Modify: `crates/netpilotd/src/lib.rs`
- Modify: `crates/netpilotd/src/state.rs`

- [ ] **Step 1: Add mpls module to lib.rs**

Replace the content of `crates/netpilotd/src/lib.rs`:

```rust
pub mod api;
pub mod cli;
pub mod mpls;
pub mod state;
```

- [ ] **Step 2: Add MplsLabelState to AppState**

Replace the content of `crates/netpilotd/src/state.rs`:

```rust
use crate::mpls::MplsLabelState;
use netpilot_config::{ConfigStore, RoutePlaneConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_store: Arc<RwLock<ConfigStore>>,
    pub mpls_labels: Arc<RwLock<MplsLabelState>>,
}

impl Default for AppState {
    fn default() -> Self {
        let default_config = RoutePlaneConfig::default();
        let mpls_labels = MplsLabelState::from_domains(
            default_config.mpls_domains.as_deref().unwrap_or(&[]),
        );
        Self {
            config_store: Arc::new(RwLock::new(ConfigStore::new(default_config))),
            mpls_labels: Arc::new(RwLock::new(mpls_labels)),
        }
    }
}
```

- [ ] **Step 3: Build check**

Run: `cargo build -p netpilotd 2>&1`
Expected: PASS

- [ ] **Step 4: Run all tests**

Run: `cargo test 2>&1`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/netpilotd/src/lib.rs crates/netpilotd/src/state.rs
git commit -m "feat: integrate MplsLabelState into AppState

Adds mpls_labels (Arc<RwLock<MplsLabelState>>) to AppState, initialized
from configured MPLS domains at startup.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: CLI show mpls labels Stub

**Files:**
- Modify: `crates/netpilotd/src/cli.rs`

- [ ] **Step 1: Add ShowMplsLabels variant to CliCommand enum**

In `crates/netpilotd/src/cli.rs`, add a new variant to the `CliCommand` enum (after `ShowMemory` on line 11):

```rust
    ShowMplsLabels,
```

The enum should now read:

```rust
    ShowStatus,
    ShowProtocols { all: bool, name: Option<String> },
    ShowInterfaces { summary: bool },
    ShowRoute { prefix: Option<String>, table: Option<String>, filter: Option<String>, filtered: bool, count: bool },
    ShowSymbols { kind: Option<String> },
    ShowBfdSessions,
    ShowRpkI,
    ShowMemory,
    ShowMplsLabels,
```

- [ ] **Step 2: Parse the new command**

In `parse_show` (line 88), add a new match arm after `Some("memory") =>` (line 109):

```rust
        Some("mpls") if parts.get(1) == Some(&"labels") => CliCommand::ShowMplsLabels,
```

The `parse_show` function should now have this added before the catch-all arm:

```rust
        Some("bfd") => CliCommand::ShowBfdSessions,
        Some("rpki") => CliCommand::ShowRpkI,
        Some("memory") => CliCommand::ShowMemory,
        Some("mpls") if parts.get(1) == Some(&"labels") => CliCommand::ShowMplsLabels,
        _ => CliCommand::Unknown(format!("show {}", parts.join(" "))),
```

- [ ] **Step 3: Execute stub**

In `execute_command` (line 131), add a match arm for the new variant before the `CliCommand::Unknown` arm (before line 187):

```rust
        CliCommand::ShowMplsLabels => {
            "show mpls labels: no MPLS table routes loaded yet\n".to_string()
        }
```

- [ ] **Step 4: Run CLI tests**

Run: `cargo test -p netpilotd 2>&1`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/netpilotd/src/cli.rs
git commit -m "feat: add 'show mpls labels' CLI command stub

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: API Integration Test for MPLS Config

**Files:**
- Modify: `crates/netpilotd/tests/api_config.rs`

- [ ] **Step 1: Add MPLS config API integration test**

Append the following test at the end of `crates/netpilotd/tests/api_config.rs`:

```rust
#[tokio::test]
async fn config_with_mpls_domains_and_tables_commits_via_api() {
    use netpilot_config::{MplsDomain, MplsLabelRange, MplsTableConfig};

    let app = build_router(AppState::default());
    let candidate = RoutePlaneConfig {
        identity: netpilot_config::RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange {
                low: 16,
                high: 1023,
            }],
            label_policy: None,
            max_label_stack_depth: Some(8),
            sr_enabled: None,
            sr_global_block: None,
            static_bindings: None,
        }]),
        mpls_tables: Some(vec![MplsTableConfig {
            name: "mpls-table-1".into(),
            domain: "main".into(),
            gc_threshold: Some(500),
            gc_period_secs: Some(60),
            sorted: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }]),
        ..RoutePlaneConfig::default()
    };

    // PUT candidate config
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/candidate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&candidate).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // GET candidate config
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/config/candidate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let config: RoutePlaneConfig = serde_json::from_slice(&body).expect("MPLS config deserializes");

    let domains = config.mpls_domains.expect("mpls_domains should be present");
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name, "main");
    assert_eq!(domains[0].label_ranges[0].low, 16);

    let tables = config.mpls_tables.expect("mpls_tables should be present");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].domain, "main");
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test -p netpilotd --test api_config 2>&1`
Expected: All tests PASS (3 tests — 2 existing + 1 new)

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1`
Expected: All tests PASS, 0 failures

- [ ] **Step 4: Commit**

```bash
git add crates/netpilotd/tests/api_config.rs
git commit -m "test: add MPLS config commit via API integration test

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Final Verification

Run the complete test suite and fmt:

```bash
cargo fmt --check
cargo test
```

Expected: clean fmt, all tests pass (approximately 45+ tests across all crates).
