use crate::schema::RoutePlaneConfig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub changed: bool,
    pub summary: Vec<String>,
}

impl ConfigDiff {
    pub fn between(running: &RoutePlaneConfig, candidate: &RoutePlaneConfig) -> Self {
        let mut summary = Vec::new();

        if running.identity != candidate.identity {
            summary.push("identity changed".to_string());
        }

        if running.tables.len() != candidate.tables.len() {
            summary.push(format!(
                "table count: {} -> {}",
                running.tables.len(),
                candidate.tables.len()
            ));
        }

        if running.protocols.len() != candidate.protocols.len() {
            summary.push(format!(
                "protocol count: {} -> {}",
                running.protocols.len(),
                candidate.protocols.len()
            ));
        }

        if running != candidate && summary.is_empty() {
            summary.push("configuration changed".to_string());
        }

        Self {
            changed: running != candidate,
            summary,
        }
    }
}
