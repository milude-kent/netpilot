use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub changed: bool,
    pub summary: Vec<String>,
}
