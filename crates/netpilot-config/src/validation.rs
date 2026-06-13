use crate::schema::{
    MplsLabelRange, ProtocolConfig, RoutePlaneConfig, SrAdjSidType, SrSidType, StaticNexthopType,
};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("{0}")]
    Message(String),
}

pub fn validate_config(config: &RoutePlaneConfig) -> Result<ValidationReport, ValidationError> {
    let mut warnings = Vec::new();

    if config.schema_version != 1 {
        return Err(ValidationError::Message(format!(
            "unsupported schema version {}",
            config.schema_version
        )));
    }

    if config.identity.router_id.trim().is_empty() {
        warnings.push("router-id is empty; dynamic protocols may require it".to_string());
    }

    let table_names = config
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .collect::<HashSet<_>>();

    for protocol in &config.protocols {
        let table = match protocol {
            ProtocolConfig::Static { table, .. } => table,
            ProtocolConfig::Bgp { table, .. } => table,
            ProtocolConfig::Ospf { table, .. } => table,
        };

        if !table_names.contains(table.as_str()) {
            return Err(ValidationError::Message(format!(
                "protocol references missing table '{table}'"
            )));
        }

        // Warn on static routes with combinations of blackhole/unreachable/prohibit
        if let ProtocolConfig::Static { routes, .. } = protocol {
            for route in routes {
                if let Some(nexthop_type) = &route.nexthop_type {
                    match nexthop_type {
                        StaticNexthopType::Blackhole
                        | StaticNexthopType::Unreachable
                        | StaticNexthopType::Prohibit => {
                            if route.next_hop.is_some() {
                                warnings.push(format!(
                                    "static route {} has nexthop-type '{nexthop_type:?}' and next-hop set; \
                                     next-hop is ignored for this nexthop type",
                                    route.prefix
                                ));
                            }
                            if route.blackhole && *nexthop_type != StaticNexthopType::Blackhole {
                                warnings.push(format!(
                                    "static route {} has blackhole=true but nexthop-type is '{:?}'; \
                                     consider using nexthop-type instead",
                                    route.prefix, nexthop_type
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // MPLS validation
    let mpls_warnings = validate_mpls(config)?;
    warnings.extend(mpls_warnings);

    // SR validation
    let sr_warnings = validate_sr(config)?;
    warnings.extend(sr_warnings);

    Ok(ValidationReport { warnings })
}

fn validate_mpls(config: &RoutePlaneConfig) -> Result<Vec<String>, ValidationError> {
    let warnings = Vec::new();

    // Collect domain names (empty set if no domains defined)
    let mut domain_names: HashSet<&str> = HashSet::new();
    if let Some(domains) = &config.mpls_domains {
        // 1. Domain name uniqueness
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
    }

    // 6. MPLS table domain references (run regardless of whether domains exist)
    if let Some(tables) = &config.mpls_tables {
        for table in tables {
            if !domain_names.contains(table.domain.as_str()) {
                return Err(ValidationError::Message(format!(
                    "MPLS table '{}' references non-existent domain '{}'",
                    table.name, table.domain
                )));
            }
        }
    }

    // 7. MPLS channel table references (run regardless of whether tables exist)
    let table_names: HashSet<&str> = config
        .mpls_tables
        .as_ref()
        .map(|tables| tables.iter().map(|t| t.name.as_str()).collect())
        .unwrap_or_default();

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

    Ok(warnings)
}

fn validate_sr(config: &RoutePlaneConfig) -> Result<Vec<String>, ValidationError> {
    let warnings = Vec::new();

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
            let domain = domain_map.get(sid.domain.as_str()).ok_or_else(|| {
                ValidationError::Message(format!(
                    "SR prefix-SID for '{}' references non-existent domain '{}'",
                    sid.prefix, sid.domain
                ))
            })?;

            match &sid.sid_type {
                SrSidType::Absolute(label) => {
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

    // Adjacency-SID domain references + absolute SID in SRGB
    if let Some(sids) = &config.sr_adjacency_sids {
        for sid in sids {
            if !domain_map.contains_key(sid.domain.as_str()) {
                return Err(ValidationError::Message(format!(
                    "SR adjacency-SID for '{}' on '{}' references non-existent domain '{}'",
                    sid.neighbor, sid.interface, sid.domain
                )));
            }
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

    // Srv6 locator validation
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

    // Srv6 SID validation
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
