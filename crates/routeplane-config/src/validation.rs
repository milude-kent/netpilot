use crate::schema::{ProtocolConfig, RoutePlaneConfig};
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
        };

        if !table_names.contains(table.as_str()) {
            return Err(ValidationError::Message(format!(
                "protocol references missing table '{table}'"
            )));
        }
    }

    Ok(ValidationReport { warnings })
}
