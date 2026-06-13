# NetPilot M5 — SR-MPLS & SRv6 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Segment Routing configuration schema (#317: SR-MPLS prefix-SID, adjacency-SID, SRGB) and (#318: SRv6 End/End.X/End.T/End.DT4/End.DT6 SIDs, locator validation), with SidRegistry runtime and compute_label_stack seed.

**Architecture:** SR-MPLS types extend MplsDomain with prefix-SID and adjacency-SID configs; SRv6 types define tagged-enum SIDs referencing locators. SidRegistry loads config into in-memory registry for prefix-SID resolution; compute_label_stack is a pure function with no IGP dependency.

**Tech Stack:** Rust 2024 edition, serde (tag/untagged enums), thiserror

---

### Task 1: SR Schema Types

**Files:**
- Modify: `crates/netpilot-config/src/schema.rs`

- [ ] **Step 1: Add SR-MPLS and SRv6 types**

Append these types at the end of `crates/netpilot-config/src/schema.rs`, after the last existing type definition (after the `Srv6LocatorConfig` closing `}`):

```rust
// ── SR-MPLS ────────────────────────────────────────────────

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

// ── SRv6 ───────────────────────────────────────────────────

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

- [ ] **Step 2: Add new fields to RoutePlaneConfig**

In the `RoutePlaneConfig` struct, add three new fields before the closing `}`:

```rust
    pub sr_prefix_sids: Option<Vec<SrPrefixSidConfig>>,
    pub sr_adjacency_sids: Option<Vec<SrAdjacencySidConfig>>,
    pub srv6_sids: Option<Vec<Srv6SidConfig>>,
```

In the `RoutePlaneConfig::default()` impl, add corresponding `None` entries before the closing `}`:

```rust
            sr_prefix_sids: None,
            sr_adjacency_sids: None,
            srv6_sids: None,
```

- [ ] **Step 3: Build check**

Run: `cargo build -p netpilot-config 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/netpilot-config/src/schema.rs
git commit -m "feat: add SR-MPLS and SRv6 schema types (#317-#318)

Adds SrPrefixSidConfig, SrAdjacencySidConfig, SrSidType, SrAdjSidType,
SrPrefixSidFlags, and Srv6SidConfig. Wires sr_prefix_sids,
sr_adjacency_sids, and srv6_sids into RoutePlaneConfig.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Update lib.rs Re-exports

**Files:**
- Modify: `crates/netpilot-config/src/lib.rs`

- [ ] **Step 1: Re-export new SR types**

Replace the existing `pub use schema::{...}` line to add the new SR types:

```rust
pub use schema::{
    AddressFamily, AuthAlgorithm, AuthPassword, BgpNeighbor, ChannelLimits, CliSocketConfig,
    ConstantDef, GrMode, LimitAction, LinkBandwidth, MplsChannelConfig, MplsDomain,
    MplsLabelPolicy, MplsLabelRange, MplsStaticBinding, MplsTableConfig, NettypeDef,
    OspfAreaConfig, ProtocolConfig, RoutePlaneConfig, RouterIdentity, SrAdjacencySidConfig,
    SrAdjSidType, SrPrefixSidConfig, SrPrefixSidFlags, SrSidType, Srv6LocatorConfig,
    Srv6SidConfig, StaticNexthopType, StaticRoute, TableConfig, TemplateRef,
};
```

- [ ] **Step 2: Build check**

Run: `cargo build -p netpilot-config 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/netpilot-config/src/lib.rs
git commit -m "feat: re-export SR schema types from netpilot-config

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: SR Validation Rules

**Files:**
- Modify: `crates/netpilot-config/src/validation.rs`

- [ ] **Step 1: Add import for SR types**

Update the import line to include `SrAdjSidType` and `SrSidType`:

```rust
use crate::schema::{
    MplsLabelRange, ProtocolConfig, RoutePlaneConfig, SrAdjSidType, SrSidType, StaticNexthopType,
};
```

- [ ] **Step 2: Add validate_sr function**

Append this function after the `validate_mpls` function (after its closing `}`):

```rust
fn validate_sr(config: &RoutePlaneConfig) -> Result<Vec<String>, ValidationError> {
    let warnings = Vec::new();

    // Resolve domain map for SRGB/SID validation
    let domain_map: std::collections::HashMap<&str, &crate::schema::MplsDomain> =
        config
            .mpls_domains
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|d| (d.name.as_str(), d))
            .collect();

    // 1. SRGB must be in domain label ranges
    for domain in config.mpls_domains.as_deref().unwrap_or(&[]) {
        if let Some(ref srgb) = domain.sr_global_block {
            let in_range = domain
                .label_ranges
                .iter()
                .any(|r| srgb.low >= r.low && srgb.high <= r.high);
            if !in_range {
                return Err(ValidationError::Message(format!(
                    "MPLS domain '{}': sr_global_block [{}, {}] is not contained within any label range",
                    domain.name, srgb.low, srgb.high
                )));
            }
        }
        // 2. SR enabled requires SRGB
        if domain.sr_enabled == Some(true) && domain.sr_global_block.is_none() {
            return Err(ValidationError::Message(format!(
                "MPLS domain '{}': sr_enabled is true but sr_global_block is not set",
                domain.name
            )));
        }
    }

    // 3-6. Prefix-SID validation
    if let Some(sids) = &config.sr_prefix_sids {
        for sid in sids {
            // Domain reference
            let domain = domain_map.get(sid.domain.as_str()).ok_or_else(|| {
                ValidationError::Message(format!(
                    "SR prefix-SID for '{}' references non-existent domain '{}'",
                    sid.prefix, sid.domain
                ))
            })?;

            match &sid.sid_type {
                SrSidType::Absolute(label) => {
                    // Must be in SRGB if SRGB is set
                    if let Some(ref srgb) = domain.sr_global_block {
                        if *label < srgb.low || *label > srgb.high {
                            return Err(ValidationError::Message(format!(
                                "SR prefix-SID '{}': absolute label {} outside domain '{}' SRGB [{}, {}]",
                                sid.prefix, label, sid.domain, srgb.low, srgb.high
                            )));
                        }
                    }
                }
                SrSidType::Index(idx) => {
                    // Index must not overflow SRGB
                    if let Some(ref srgb) = domain.sr_global_block {
                        if srgb.low + idx > srgb.high {
                            return Err(ValidationError::Message(format!(
                                "SR prefix-SID '{}': index {} overflows domain '{}' SRGB [{}, {}]",
                                sid.prefix, idx, sid.domain, srgb.low, srgb.high
                            )));
                        }
                    }
                }
            }
        }
    }

    // Adjacency-SID domain references
    if let Some(sids) = &config.sr_adjacency_sids {
        for sid in sids {
            if !domain_map.contains_key(sid.domain.as_str()) {
                return Err(ValidationError::Message(format!(
                    "SR adjacency-SID for '{}' on '{}' references non-existent domain '{}'",
                    sid.neighbor, sid.interface, sid.domain
                )));
            }
            // Absolute adjacency-SID in SRGB check
            if let SrAdjSidType::Absolute(label) = sid.sid_type {
                if let Some(domain) = domain_map.get(sid.domain.as_str()) {
                    if let Some(ref srgb) = domain.sr_global_block {
                        if label < srgb.low || label > srgb.high {
                            return Err(ValidationError::Message(format!(
                                "SR adjacency-SID: absolute label {} outside domain '{}' SRGB [{}, {}]",
                                label, sid.domain, srgb.low, srgb.high
                            )));
                        }
                    }
                }
            }
        }
    }

    // 7. Srv6 locator validation
    let locator_map: std::collections::HashMap<&str, &crate::schema::Srv6LocatorConfig> =
        config
            .srv6_locators
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|l| (l.name.as_str(), l))
            .collect();

    for locator in config.srv6_locators.as_deref().unwrap_or(&[]) {
        let total = locator.block_len.unwrap_or(0) as u32
            + locator.node_len.unwrap_or(0) as u32
            + locator.function_len.unwrap_or(0) as u32;
        if total > 128 {
            return Err(ValidationError::Message(format!(
                "SRv6 locator '{}': block_len + node_len + function_len = {} exceeds 128",
                locator.name, total
            )));
        }
    }

    // 8. Srv6 SID validation
    if let Some(sids) = &config.srv6_sids {
        for sid in sids {
            let (name, locator_name, function) = match sid {
                crate::schema::Srv6SidConfig::End { name, locator, function } => (name, locator, function),
                crate::schema::Srv6SidConfig::EndX { name, locator, function, .. } => (name, locator, function),
                crate::schema::Srv6SidConfig::EndT { name, locator, function, .. } => (name, locator, function),
                crate::schema::Srv6SidConfig::EndDT4 { name, locator, function, .. } => (name, locator, function),
                crate::schema::Srv6SidConfig::EndDT6 { name, locator, function, .. } => (name, locator, function),
            };

            let locator = locator_map.get(locator_name.as_str()).ok_or_else(|| {
                ValidationError::Message(format!(
                    "SRv6 SID '{}' references non-existent locator '{}'",
                    name, locator_name
                ))
            })?;

            // Function must fit within locator's function_len bits
            if let Some(func_len) = locator.function_len {
                let max_func = (1u32 << func_len) - 1;
                if *function > max_func {
                    return Err(ValidationError::Message(format!(
                        "SRv6 SID '{}': function {} exceeds max {} for locator '{}' (function_len={})",
                        name, function, max_func, locator_name, func_len
                    )));
                }
            }
        }
    }

    Ok(warnings)
}
```

- [ ] **Step 3: Wire validate_sr into validate_config**

In `validate_config`, find the MPLS validation call section:
```rust
    // MPLS validation
    let mpls_warnings = validate_mpls(config)?;
    warnings.extend(mpls_warnings);

    Ok(ValidationReport { warnings })
}
```

Replace with:
```rust
    // MPLS validation
    let mpls_warnings = validate_mpls(config)?;
    warnings.extend(mpls_warnings);

    // SR validation
    let sr_warnings = validate_sr(config)?;
    warnings.extend(sr_warnings);

    Ok(ValidationReport { warnings })
}
```

- [ ] **Step 4: Build + test check**

Run: `cargo build -p netpilot-config 2>&1 && cargo test -p netpilot-config 2>&1`
Expected: PASS (all 23 existing tests still pass)

- [ ] **Step 5: Commit**

```bash
git add crates/netpilot-config/src/validation.rs
git commit -m "feat: add SR validation rules — SRGB bounds, SID references, SRv6 function limits (#317-#318)

10 validation rules: SRGB in domain range, sr_enabled requires SRGB,
prefix-SID domain reference, adjacency-SID domain reference, absolute SID
in SRGB, index SID overflow, adjacency-SID absolute in SRGB, SRv6 locator
length sum, SRv6 SID function bounds, SRv6 SID locator reference.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: SR Schema Round-trip + Validation Tests

**Files:**
- Modify: `crates/netpilot-config/tests/config_store.rs`

- [ ] **Step 1: Add SR test imports**

Update the file's imports to include SR types. Change the existing MPLS import block to also include SR types:

```rust
use netpilot_config::{
    MplsChannelConfig, MplsDomain, MplsLabelPolicy, MplsLabelRange, MplsStaticBinding,
    MplsTableConfig, SrAdjacencySidConfig, SrAdjSidType, SrPrefixSidConfig, SrPrefixSidFlags,
    SrSidType, Srv6SidConfig,
};
```

(The existing imports for AddressFamily, CommitRequest, ConfigStore, etc. remain unchanged.)

- [ ] **Step 2: Append SR round-trip tests (9 tests)**

Append these tests at the end of the file:

```rust
// ── SR Schema Round-trip Tests ───────────────────────────────

#[test]
fn sr_prefix_sid_absolute_round_trips() {
    let sid = SrPrefixSidConfig {
        prefix: "10.0.0.0/8".into(),
        domain: "main".into(),
        sid_type: SrSidType::Absolute(16000),
        flags: SrPrefixSidFlags {
            n_flag_clear: None,
            php: Some(true),
            explicit_null: None,
        },
    };
    let encoded = serde_json::to_string(&sid).expect("serializes");
    let decoded: SrPrefixSidConfig = serde_json::from_str(&encoded).expect("deserializes");
    assert_eq!(decoded.prefix, "10.0.0.0/8");
    assert_eq!(decoded.domain, "main");
    assert!(matches!(decoded.sid_type, SrSidType::Absolute(16000)));
    assert_eq!(decoded.flags.php, Some(true));
}

#[test]
fn sr_prefix_sid_index_round_trips() {
    let sid = SrPrefixSidConfig {
        prefix: "192.168.0.0/16".into(),
        domain: "main".into(),
        sid_type: SrSidType::Index(5),
        flags: SrPrefixSidFlags {
            n_flag_clear: Some(true),
            php: None,
            explicit_null: Some(true),
        },
    };
    let encoded = serde_json::to_string(&sid).expect("serializes");
    let decoded: SrPrefixSidConfig = serde_json::from_str(&encoded).expect("deserializes");
    assert!(matches!(decoded.sid_type, SrSidType::Index(5)));
    assert_eq!(decoded.flags.n_flag_clear, Some(true));
    assert_eq!(decoded.flags.explicit_null, Some(true));
}

#[test]
fn sr_prefix_sid_flags_all_set_round_trips() {
    let flags = SrPrefixSidFlags {
        n_flag_clear: Some(true),
        php: Some(true),
        explicit_null: Some(true),
    };
    let encoded = serde_json::to_string(&flags).expect("serializes");
    let decoded: SrPrefixSidFlags = serde_json::from_str(&encoded).expect("deserializes");
    assert_eq!(decoded.n_flag_clear, Some(true));
    assert_eq!(decoded.php, Some(true));
    assert_eq!(decoded.explicit_null, Some(true));
}

#[test]
fn sr_adjacency_sid_absolute_round_trips() {
    let sid = SrAdjacencySidConfig {
        interface: "eth0".into(),
        neighbor: "192.0.2.1".into(),
        domain: "main".into(),
        sid_type: SrAdjSidType::Absolute(17000),
        protected: true,
    };
    let encoded = serde_json::to_string(&sid).expect("serializes");
    let decoded: SrAdjacencySidConfig = serde_json::from_str(&encoded).expect("deserializes");
    assert_eq!(decoded.interface, "eth0");
    assert_eq!(decoded.neighbor, "192.0.2.1");
    assert!(matches!(decoded.sid_type, SrAdjSidType::Absolute(17000)));
    assert!(decoded.protected);
}

#[test]
fn sr_adjacency_sid_dynamic_round_trips() {
    let sid = SrAdjacencySidConfig {
        interface: "eth1".into(),
        neighbor: "2001:db8::1".into(),
        domain: "main".into(),
        sid_type: SrAdjSidType::Dynamic,
        protected: false,
    };
    let encoded = serde_json::to_string(&sid).expect("serializes");
    let decoded: SrAdjacencySidConfig = serde_json::from_str(&encoded).expect("deserializes");
    assert!(matches!(decoded.sid_type, SrAdjSidType::Dynamic));
    assert!(!decoded.protected);
}

#[test]
fn srv6_sid_end_round_trips() {
    let sid = Srv6SidConfig::End {
        name: "end1".into(),
        locator: "loc1".into(),
        function: 1,
    };
    let encoded = serde_json::to_string(&sid).expect("serializes");
    assert!(encoded.contains("end"));
    let decoded: Srv6SidConfig = serde_json::from_str(&encoded).expect("deserializes");
    match decoded {
        Srv6SidConfig::End { name, locator, function } => {
            assert_eq!(name, "end1");
            assert_eq!(locator, "loc1");
            assert_eq!(function, 1);
        }
        _ => panic!("expected End variant"),
    }
}

#[test]
fn srv6_sid_endx_round_trips() {
    let sid = Srv6SidConfig::EndX {
        name: "endx1".into(),
        locator: "loc1".into(),
        function: 2,
        interface: "eth0".into(),
        nexthop: "2001:db8::1".into(),
    };
    let encoded = serde_json::to_string(&sid).expect("serializes");
    assert!(encoded.contains("end-x"));
    let decoded: Srv6SidConfig = serde_json::from_str(&encoded).expect("deserializes");
    match decoded {
        Srv6SidConfig::EndX { interface, nexthop, .. } => {
            assert_eq!(interface, "eth0");
            assert_eq!(nexthop, "2001:db8::1");
        }
        _ => panic!("expected EndX variant"),
    }
}

#[test]
fn srv6_sid_end_dt4_round_trips() {
    let sid = Srv6SidConfig::EndDT4 {
        name: "dt4-1".into(),
        locator: "loc1".into(),
        function: 100,
        vrf: "vrf-red".into(),
    };
    let encoded = serde_json::to_string(&sid).expect("serializes");
    assert!(encoded.contains("end-dt4"));
    let decoded: Srv6SidConfig = serde_json::from_str(&encoded).expect("deserializes");
    match decoded {
        Srv6SidConfig::EndDT4 { vrf, .. } => assert_eq!(vrf, "vrf-red"),
        _ => panic!("expected EndDT4 variant"),
    }
}

#[test]
fn full_config_with_sr_round_trips() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: Some(MplsLabelRange { low: 16000, high: 24000 }),
            static_bindings: None,
        }]),
        sr_prefix_sids: Some(vec![SrPrefixSidConfig {
            prefix: "10.0.0.0/8".into(),
            domain: "main".into(),
            sid_type: SrSidType::Index(0),
            flags: SrPrefixSidFlags {
                n_flag_clear: None,
                php: None,
                explicit_null: None,
            },
        }]),
        srv6_locators: Some(vec![netpilot_config::Srv6LocatorConfig {
            name: "loc1".into(),
            prefix: "2001:db8:1::/48".into(),
            block_len: Some(32),
            node_len: Some(16),
            function_len: Some(16),
        }]),
        srv6_sids: Some(vec![Srv6SidConfig::End {
            name: "end1".into(),
            locator: "loc1".into(),
            function: 1,
        }]),
        ..RoutePlaneConfig::default()
    };

    let encoded = serde_json::to_string(&config).expect("serializes");
    let decoded: RoutePlaneConfig = serde_json::from_str(&encoded).expect("deserializes");

    let sids = decoded.sr_prefix_sids.expect("sr_prefix_sids present");
    assert_eq!(sids.len(), 1);
    let sr_sids = decoded.srv6_sids.expect("srv6_sids present");
    assert_eq!(sr_sids.len(), 1);
}

// ── SR Validation Tests ─────────────────────────────────────

#[test]
fn validation_rejects_srgb_outside_domain_ranges() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 100, high: 199 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: Some(MplsLabelRange { low: 1000, high: 2000 }),
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("SRGB outside ranges should fail");
    assert!(err.to_string().contains("sr_global_block"));
    assert!(err.to_string().contains("not contained"));
}

#[test]
fn validation_rejects_sr_enabled_without_srgb() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: None,
            static_bindings: None,
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("sr_enabled without SRGB should fail");
    assert!(err.to_string().contains("sr_enabled"));
    assert!(err.to_string().contains("sr_global_block"));
}

#[test]
fn validation_rejects_prefix_sid_unknown_domain() {
    let config = RoutePlaneConfig {
        sr_prefix_sids: Some(vec![SrPrefixSidConfig {
            prefix: "10.0.0.0/8".into(),
            domain: "ghost".into(),
            sid_type: SrSidType::Absolute(16000),
            flags: SrPrefixSidFlags {
                n_flag_clear: None,
                php: None,
                explicit_null: None,
            },
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("unknown domain should fail");
    assert!(err.to_string().contains("non-existent domain"));
}

#[test]
fn validation_rejects_absolute_sid_outside_srgb() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: Some(MplsLabelRange { low: 16000, high: 17000 }),
            static_bindings: None,
        }]),
        sr_prefix_sids: Some(vec![SrPrefixSidConfig {
            prefix: "10.0.0.0/8".into(),
            domain: "main".into(),
            sid_type: SrSidType::Absolute(20000),
            flags: SrPrefixSidFlags {
                n_flag_clear: None,
                php: None,
                explicit_null: None,
            },
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("SID outside SRGB should fail");
    assert!(err.to_string().contains("outside domain"));
    assert!(err.to_string().contains("SRGB"));
}

#[test]
fn validation_rejects_index_sid_overflow() {
    let config = RoutePlaneConfig {
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: Some(MplsLabelRange { low: 16000, high: 16099 }),
            static_bindings: None,
        }]),
        sr_prefix_sids: Some(vec![SrPrefixSidConfig {
            prefix: "10.0.0.0/8".into(),
            domain: "main".into(),
            sid_type: SrSidType::Index(200),
            flags: SrPrefixSidFlags {
                n_flag_clear: None,
                php: None,
                explicit_null: None,
            },
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("index overflow should fail");
    assert!(err.to_string().contains("overflows"));
}

#[test]
fn validation_rejects_srv6_sid_function_exceeds_locator() {
    let config = RoutePlaneConfig {
        srv6_locators: Some(vec![netpilot_config::Srv6LocatorConfig {
            name: "loc1".into(),
            prefix: "2001:db8:1::/48".into(),
            block_len: Some(32),
            node_len: Some(16),
            function_len: Some(8),
        }]),
        srv6_sids: Some(vec![Srv6SidConfig::End {
            name: "bad".into(),
            locator: "loc1".into(),
            function: 300,
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("function exceeds locator should fail");
    assert!(err.to_string().contains("exceeds max"));
}

#[test]
fn validation_rejects_srv6_sid_unknown_locator() {
    let config = RoutePlaneConfig {
        srv6_sids: Some(vec![Srv6SidConfig::End {
            name: "orphan".into(),
            locator: "ghost-loc".into(),
            function: 1,
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("unknown locator should fail");
    assert!(err.to_string().contains("non-existent locator"));
}

#[test]
fn validation_rejects_srv6_locator_length_sum_exceeds_128() {
    let config = RoutePlaneConfig {
        srv6_locators: Some(vec![netpilot_config::Srv6LocatorConfig {
            name: "bad-loc".into(),
            prefix: "2001:db8:1::/48".into(),
            block_len: Some(64),
            node_len: Some(64),
            function_len: Some(64),
        }]),
        ..RoutePlaneConfig::default()
    };
    let err = validate_config(&config).expect_err("length sum too high should fail");
    assert!(err.to_string().contains("exceeds 128"));
}

#[test]
fn validation_accepts_valid_sr_config() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: Some(MplsLabelRange { low: 16000, high: 24000 }),
            static_bindings: None,
        }]),
        sr_prefix_sids: Some(vec![SrPrefixSidConfig {
            prefix: "10.0.0.0/8".into(),
            domain: "main".into(),
            sid_type: SrSidType::Index(0),
            flags: SrPrefixSidFlags {
                n_flag_clear: None,
                php: None,
                explicit_null: None,
            },
        }]),
        sr_adjacency_sids: Some(vec![SrAdjacencySidConfig {
            interface: "eth0".into(),
            neighbor: "192.0.2.1".into(),
            domain: "main".into(),
            sid_type: SrAdjSidType::Dynamic,
            protected: false,
        }]),
        srv6_locators: Some(vec![netpilot_config::Srv6LocatorConfig {
            name: "loc1".into(),
            prefix: "2001:db8:1::/48".into(),
            block_len: Some(32),
            node_len: Some(16),
            function_len: Some(16),
        }]),
        srv6_sids: Some(vec![Srv6SidConfig::End {
            name: "end1".into(),
            locator: "loc1".into(),
            function: 1,
        }]),
        ..RoutePlaneConfig::default()
    };
    let report = validate_config(&config).expect("valid SR config should pass");
    assert!(report.warnings.is_empty() || report.warnings.iter().all(|w| w.contains("router-id")));
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p netpilot-config 2>&1`
Expected: All tests PASS (23 existing + 20 new = 43 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/netpilot-config/tests/config_store.rs
git commit -m "test: add SR schema round-trip and validation tests (20 tests)

9 round-trip tests: prefix-SID absolute/index, flags, adjacency-SID
absolute/dynamic, End/EndX/EndDT4 SIDs, full SR config. 10 validation
tests: SRGB in range, sr_enabled without SRGB, unknown domain, absolute
SID outside SRGB, index overflow, function exceeds, unknown locator,
locator length sum, valid config.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: SidRegistry Runtime Module

**Files:**
- Create: `crates/netpilotd/src/sr.rs`

- [ ] **Step 1: Create sr.rs with SidRegistry + compute_label_stack + inline tests**

Write the full content of `crates/netpilotd/src/sr.rs`:

```rust
use netpilot_config::{RoutePlaneConfig, SrAdjSidType, SrSidType};

/// In-memory registry of SR SIDs, loaded from config.
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
    /// Build the registry from a RoutePlaneConfig, resolving Index SIDs to
    /// absolute labels using each domain's SRGB.
    pub fn from_config(config: &RoutePlaneConfig) -> Self {
        let mut registry = Self::default();

        if let Some(sids) = &config.sr_prefix_sids {
            for sid in sids {
                let label = match &sid.sid_type {
                    SrSidType::Absolute(l) => *l,
                    SrSidType::Index(idx) => resolve_index_in_srgb(config, &sid.domain, *idx),
                };
                registry.prefix_sids.push(SrPrefixSidEntry {
                    prefix: sid.prefix.clone(),
                    label,
                    domain: sid.domain.clone(),
                });
            }
        }

        if let Some(sids) = &config.sr_adjacency_sids {
            for sid in sids {
                let label = match &sid.sid_type {
                    SrAdjSidType::Absolute(l) => *l,
                    SrAdjSidType::Dynamic => 0, // placeholder; M6: allocate from domain pool
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

    /// Resolve a destination prefix to its prefix-SID label.
    /// Uses longest prefix match.
    pub fn resolve_prefix_sid(&self, prefix: &str) -> Option<u32> {
        self.prefix_sids
            .iter()
            .filter(|e| prefix_starts_with(prefix, &e.prefix))
            .max_by_key(|e| prefix_length(&e.prefix))
            .map(|e| e.label)
    }

    /// List all registered prefix-SID entries.
    pub fn list_prefix_sids(&self) -> &[SrPrefixSidEntry] {
        &self.prefix_sids
    }

    /// List all registered adjacency-SID entries.
    pub fn list_adjacency_sids(&self) -> &[SrAdjacencySidEntry] {
        &self.adjacency_sids
    }
}

/// Compute an MPLS label stack for a given destination.
///
/// In this phase, returns a single-label stack if the destination
/// matches a prefix-SID. In M6, this will incorporate IGP topology
/// and adjacency-SIDs to build multi-hop stacks.
pub fn compute_label_stack(
    registry: &SidRegistry,
    destination: &str,
) -> Option<Vec<u32>> {
    registry.resolve_prefix_sid(destination).map(|label| vec![label])
}

/// Resolve a SID index against the domain's SRGB.
/// Returns SRGB.low + index if SRGB is configured, otherwise returns
/// the index as a raw label (fallback).
fn resolve_index_in_srgb(config: &RoutePlaneConfig, domain_name: &str, index: u32) -> u32 {
    if let Some(domains) = &config.mpls_domains {
        if let Some(d) = domains.iter().find(|d| d.name == domain_name) {
            if let Some(ref srgb) = d.sr_global_block {
                return srgb.low + index;
            }
        }
    }
    index // fallback
}

/// Check whether `addr` starts with `prefix` (simplified string-based match).
fn prefix_starts_with(addr: &str, prefix: &str) -> bool {
    let prefix_base = prefix
        .split('/')
        .next()
        .unwrap_or(prefix);
    let addr_base = addr
        .split('/')
        .next()
        .unwrap_or(addr);
    addr_base == prefix_base
}

/// Extract the prefix length from a "prefix/length" string.
fn prefix_length(prefix: &str) -> usize {
    prefix
        .split('/')
        .nth(1)
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use netpilot_config::{MplsDomain, MplsLabelRange};

    fn make_test_config() -> RoutePlaneConfig {
        RoutePlaneConfig {
            mpls_domains: Some(vec![MplsDomain {
                name: "main".into(),
                label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: Some(true),
                sr_global_block: Some(MplsLabelRange { low: 16000, high: 24000 }),
                static_bindings: None,
            }]),
            sr_prefix_sids: Some(vec![
                netpilot_config::SrPrefixSidConfig {
                    prefix: "10.0.0.0/8".into(),
                    domain: "main".into(),
                    sid_type: SrSidType::Index(0),
                    flags: netpilot_config::SrPrefixSidFlags {
                        n_flag_clear: None,
                        php: None,
                        explicit_null: None,
                    },
                },
                netpilot_config::SrPrefixSidConfig {
                    prefix: "192.168.0.0/16".into(),
                    domain: "main".into(),
                    sid_type: SrSidType::Absolute(17000),
                    flags: netpilot_config::SrPrefixSidFlags {
                        n_flag_clear: None,
                        php: None,
                        explicit_null: None,
                    },
                },
            ]),
            ..RoutePlaneConfig::default()
        }
    }

    #[test]
    fn sid_registry_loads_prefix_sids_from_config() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.list_prefix_sids().len(), 2);
    }

    #[test]
    fn resolve_prefix_sid_finds_exact_match() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), Some(16000));
    }

    #[test]
    fn resolve_prefix_sid_returns_none_for_unknown() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.resolve_prefix_sid("172.16.0.0/12"), None);
    }

    #[test]
    fn compute_label_stack_returns_single_label() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        let stack = compute_label_stack(&registry, "10.0.0.0/8");
        assert_eq!(stack, Some(vec![16000]));
    }

    #[test]
    fn compute_label_stack_returns_none_for_unknown() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        let stack = compute_label_stack(&registry, "172.16.0.0/12");
        assert_eq!(stack, None);
    }

    #[test]
    fn absolute_sid_is_used_directly() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.resolve_prefix_sid("192.168.0.0/16"), Some(17000));
    }

    #[test]
    fn index_sid_resolves_against_srgb() {
        let config = make_test_config();
        let registry = SidRegistry::from_config(&config);
        // Index 0 in SRGB [16000, 24000] = 16000
        assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), Some(16000));
    }

    #[test]
    fn adjacency_sid_dynamic_sets_label_to_zero_placeholder() {
        let config = RoutePlaneConfig {
            mpls_domains: Some(vec![MplsDomain {
                name: "main".into(),
                label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
                label_policy: None,
                max_label_stack_depth: None,
                sr_enabled: Some(true),
                sr_global_block: Some(MplsLabelRange { low: 16000, high: 24000 }),
                static_bindings: None,
            }]),
            sr_adjacency_sids: Some(vec![netpilot_config::SrAdjacencySidConfig {
                interface: "eth0".into(),
                neighbor: "192.0.2.1".into(),
                domain: "main".into(),
                sid_type: SrAdjSidType::Dynamic,
                protected: false,
            }]),
            ..RoutePlaneConfig::default()
        };
        let registry = SidRegistry::from_config(&config);
        assert_eq!(registry.list_adjacency_sids().len(), 1);
        assert_eq!(registry.list_adjacency_sids()[0].label, 0);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p netpilotd --lib 2>&1`
Expected: 8 tests pass in the sr module (plus 11 existing mpls tests = 19 total)

- [ ] **Step 3: Commit**

```bash
git add crates/netpilotd/src/sr.rs
git commit -m "feat: add SidRegistry and compute_label_stack runtime (#317)

SidRegistry loads prefix/adjacency-SIDs from config, resolving Index SIDs
against domain SRGB. compute_label_stack returns single-label stack for
known prefixes (IGP integration deferred to M6). 8 unit tests.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Wire SR into AppState and lib.rs

**Files:**
- Modify: `crates/netpilotd/src/lib.rs`
- Modify: `crates/netpilotd/src/state.rs`

- [ ] **Step 1: Add sr module to lib.rs**

Replace the content of `crates/netpilotd/src/lib.rs`:

```rust
pub mod api;
pub mod cli;
pub mod mpls;
pub mod sr;
pub mod state;
```

- [ ] **Step 2: Add SidRegistry to AppState**

Replace the content of `crates/netpilotd/src/state.rs`:

```rust
use crate::mpls::MplsLabelState;
use crate::sr::SidRegistry;
use netpilot_config::{ConfigStore, RoutePlaneConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_store: Arc<RwLock<ConfigStore>>,
    pub mpls_labels: Arc<RwLock<MplsLabelState>>,
    pub sid_registry: Arc<RwLock<SidRegistry>>,
}

impl Default for AppState {
    fn default() -> Self {
        let default_config = RoutePlaneConfig::default();
        let mpls_labels = MplsLabelState::from_domains(
            default_config.mpls_domains.as_deref().unwrap_or(&[]),
        );
        let sid_registry = SidRegistry::from_config(&default_config);
        Self {
            config_store: Arc::new(RwLock::new(ConfigStore::new(default_config))),
            mpls_labels: Arc::new(RwLock::new(mpls_labels)),
            sid_registry: Arc::new(RwLock::new(sid_registry)),
        }
    }
}
```

- [ ] **Step 3: Build check + run tests**

Run: `cargo build -p netpilotd 2>&1 && cargo test -p netpilotd 2>&1`
Expected: PASS, all tests passing

- [ ] **Step 4: Commit**

```bash
git add crates/netpilotd/src/lib.rs crates/netpilotd/src/state.rs
git commit -m "feat: integrate SidRegistry into AppState

Adds sid_registry (Arc<RwLock<SidRegistry>>) to AppState, initialized
from configured SR prefix/adjacency-SIDs at startup.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: CLI SR Stubs

**Files:**
- Modify: `crates/netpilotd/src/cli.rs`

- [ ] **Step 1: Add CLI variants**

Add two new variants to the `CliCommand` enum (after `ShowMplsLabels`):

```rust
    ShowSrPrefixSids,
    ShowSrv6Sids,
```

- [ ] **Step 2: Parse the new commands**

In `parse_show`, add after the `ShowMplsLabels` match arm:

```rust
        Some("sr") if parts.get(1) == Some(&"prefix-sids") => CliCommand::ShowSrPrefixSids,
        Some("srv6") if parts.get(1) == Some(&"sids") => CliCommand::ShowSrv6Sids,
```

- [ ] **Step 3: Execute stubs**

In `execute_command`, add after the `ShowMplsLabels` match arm:

```rust
        CliCommand::ShowSrPrefixSids => {
            "show sr prefix-sids: no IGP topology loaded yet\n".to_string()
        }
        CliCommand::ShowSrv6Sids => {
            "show srv6 sids: no SRv6 dataplane configured yet\n".to_string()
        }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p netpilotd 2>&1`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/netpilotd/src/cli.rs
git commit -m "feat: add 'show sr prefix-sids' and 'show srv6 sids' CLI stubs

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: SR Runtime Integration Test

**Files:**
- Create: `crates/netpilotd/tests/sr.rs`

- [ ] **Step 1: Create SR integration test file**

Write `crates/netpilotd/tests/sr.rs`:

```rust
use netpilot_config::{
    MplsDomain, MplsLabelRange, RoutePlaneConfig, RouterIdentity, SrPrefixSidConfig,
    SrPrefixSidFlags, SrSidType,
};
use netpilotd::sr::SidRegistry;

#[test]
fn sid_registry_from_config_with_srgb_resolves_index_correctly() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        mpls_domains: Some(vec![MplsDomain {
            name: "main".into(),
            label_ranges: vec![MplsLabelRange { low: 16000, high: 24000 }],
            label_policy: None,
            max_label_stack_depth: None,
            sr_enabled: Some(true),
            sr_global_block: Some(MplsLabelRange { low: 16000, high: 24000 }),
            static_bindings: None,
        }]),
        sr_prefix_sids: Some(vec![
            SrPrefixSidConfig {
                prefix: "10.0.0.0/8".into(),
                domain: "main".into(),
                sid_type: SrSidType::Index(1),
                flags: SrPrefixSidFlags {
                    n_flag_clear: None,
                    php: None,
                    explicit_null: None,
                },
            },
            SrPrefixSidConfig {
                prefix: "172.16.0.0/12".into(),
                domain: "main".into(),
                sid_type: SrSidType::Absolute(17000),
                flags: SrPrefixSidFlags {
                    n_flag_clear: None,
                    php: Some(true),
                    explicit_null: None,
                },
            },
        ]),
        ..RoutePlaneConfig::default()
    };

    let registry = SidRegistry::from_config(&config);

    // Index-based SID resolves as SRGB.low + index = 16000 + 1 = 16001
    assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), Some(16001));
    // Absolute SID used directly
    assert_eq!(registry.resolve_prefix_sid("172.16.0.0/12"), Some(17000));
    // Unknown prefix
    assert_eq!(registry.resolve_prefix_sid("192.168.0.0/16"), None);
}

#[test]
fn empty_config_creates_empty_registry() {
    let config = RoutePlaneConfig::default();
    let registry = SidRegistry::from_config(&config);
    assert!(registry.list_prefix_sids().is_empty());
    assert!(registry.list_adjacency_sids().is_empty());
    assert_eq!(registry.resolve_prefix_sid("10.0.0.0/8"), None);
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test -p netpilotd --test sr 2>&1`
Expected: 2 tests PASS

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1`
Expected: All tests PASS, 0 failures

- [ ] **Step 4: Commit**

```bash
git add crates/netpilotd/tests/sr.rs
git commit -m "test: add SidRegistry integration tests

2 tests: SRGB-based index resolution, empty config empty registry.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Final Verification

```bash
cargo fmt --check
cargo test
```

Expected: clean fmt, all tests pass (approximately 165+ tests across all crates).
