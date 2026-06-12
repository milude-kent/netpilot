use crate::schema::{ProtocolConfig, RoutePlaneConfig, StaticNexthopType};
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

    Ok(ValidationReport { warnings })
}
