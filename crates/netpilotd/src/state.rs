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
