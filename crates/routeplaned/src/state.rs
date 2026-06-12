use routeplane_config::{ConfigStore, RoutePlaneConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_store: Arc<RwLock<ConfigStore>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config_store: Arc::new(RwLock::new(ConfigStore::new(RoutePlaneConfig::default()))),
        }
    }
}
