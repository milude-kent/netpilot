use crate::mpls::MplsLabelState;
use crate::sr::SidRegistry;
use netpilot_config::{ConfigStore, RoutePlaneConfig};
use netpilot_protocol::ProtocolSupervisor;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_store: Arc<RwLock<ConfigStore>>,
    pub mpls_labels: Arc<RwLock<MplsLabelState>>,
    pub sid_registry: Arc<RwLock<SidRegistry>>,
    pub supervisor: Arc<RwLock<ProtocolSupervisor>>,
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
            supervisor: Arc::new(RwLock::new(ProtocolSupervisor::new())),
        }
    }
}
