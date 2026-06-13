use crate::schema::{MplsLabelRange, ProtocolConfig, RoutePlaneConfig, StaticNexthopType};
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

    Ok(ValidationReport { warnings })
}

fn validate_mpls(config: &RoutePlaneConfig) -> Result<Vec<String>, ValidationError> {
    let warnings = Vec::new();

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
