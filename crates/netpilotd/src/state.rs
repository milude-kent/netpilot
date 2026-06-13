use crate::mpls::MplsLabelState;
use crate::sr::SidRegistry;
use metrics_exporter_prometheus::PrometheusHandle;
use netpilot_config::{AuthConfig, CommitScheduler, ConfigStore, RoutePlaneConfig};
use netpilot_protocol::{ProtocolEvent, ProtocolSupervisor};
use netpilot_rib::RibCore;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

#[derive(Clone)]
pub struct AppState {
    pub config_store: Arc<RwLock<ConfigStore>>,
    /// Async coordinator for timed `commit-confirmed` operations. Wraps the
    /// shared `ConfigStore` and spawns the auto-rollback task that fires
    /// when an operator does not confirm a commit within the configured
    /// timeout.
    pub scheduler: Arc<CommitScheduler>,
    pub mpls_labels: Arc<RwLock<MplsLabelState>>,
    pub sid_registry: Arc<RwLock<SidRegistry>>,
    pub supervisor: Arc<RwLock<ProtocolSupervisor>>,
    pub rib: Arc<RwLock<RibCore>>,
    /// Prometheus handle used by the `/metrics` route. The recorder is
    /// installed process-wide by [`crate::metrics::install_recorder`].
    pub metrics: Arc<PrometheusHandle>,
    /// Clonable broadcast sender for supervisor protocol events. Held
    /// outside of the supervisor's `RwLock` so that gRPC `Subscribe`
    /// (stream mode) and the SSE handler can subscribe cheaply without
    /// taking the supervisor lock on every event.
    pub event_tx: Arc<broadcast::Sender<ProtocolEvent>>,
    /// Auth / TLS configuration for the control plane. When `None`, the
    /// control plane is unauthenticated and uses plain HTTP. When
    /// `Some(_)`, [`crate::auth_mw::bearer_auth_middleware`] enforces
    /// bearer-token validation on every non-allowlisted route, and
    /// `main.rs` will bind a TLS listener if cert + key paths are set.
    pub auth: Arc<RwLock<AuthConfig>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::with_metrics(crate::metrics::install_recorder())
    }
}

impl AppState {
    /// Build an `AppState` with a specific metrics handle. Tests that need
    /// to install their own recorder should use this constructor.
    pub fn with_metrics(metrics: PrometheusHandle) -> Self {
        Self::with_metrics_and_auth(metrics, AuthConfig::default())
    }

    /// Build an `AppState` with explicit metrics and auth config. Tests
    /// that need to exercise the auth middleware use this to install a
    /// known bearer secret.
    pub fn with_metrics_and_auth(metrics: PrometheusHandle, auth: AuthConfig) -> Self {
        let default_config = RoutePlaneConfig::default();
        let mpls_labels =
            MplsLabelState::from_domains(default_config.mpls_domains.as_deref().unwrap_or(&[]));
        let sid_registry = SidRegistry::from_config(&default_config);
        let supervisor = ProtocolSupervisor::new();
        let event_tx = supervisor.event_sender();
        let config_store = Arc::new(RwLock::new(ConfigStore::new(default_config)));
        let scheduler = Arc::new(CommitScheduler::new(config_store.clone()));
        Self {
            config_store,
            scheduler,
            mpls_labels: Arc::new(RwLock::new(mpls_labels)),
            sid_registry: Arc::new(RwLock::new(sid_registry)),
            supervisor: Arc::new(RwLock::new(supervisor)),
            rib: Arc::new(RwLock::new(RibCore::new())),
            metrics: Arc::new(metrics),
            event_tx: Arc::new(event_tx),
            auth: Arc::new(RwLock::new(auth)),
        }
    }
}
