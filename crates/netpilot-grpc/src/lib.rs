pub mod gnmi_svc;
pub mod netpilot_svc;
pub mod path_resolver;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// State shared with gRPC services. Only holds config_store.
/// netpilotd wires this via Arc from its own AppState.
#[derive(Clone)]
pub struct GrpcAppState {
    pub config_store: Arc<RwLock<netpilot_config::ConfigStore>>,
}

impl GrpcAppState {
    pub fn new(config_store: Arc<RwLock<netpilot_config::ConfigStore>>) -> Self {
        Self { config_store }
    }
}

/// Start the gRPC server. Returns a future that resolves when the server stops.
pub async fn serve(
    addr: SocketAddr,
    state: GrpcAppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tonic::transport::Server;

    let gnmi_svc = gnmi_svc::GnmiService::new(state.clone());
    let config_svc = netpilot_svc::ConfigService::new(state.clone());
    let health_svc = netpilot_svc::HealthService::new(state);

    Server::builder()
        .add_service(gnmi_svc.into_gnmi_server())
        .add_service(config_svc.into_config_server())
        .add_service(health_svc.into_health_server())
        .serve(addr)
        .await?;

    Ok(())
}

/// Proto-generated modules. These are at the crate root because tonic::include_proto!
/// generates nested modules from dotted names.
pub mod gnmi {
    tonic::include_proto!("gnmi.v1");
}
pub mod netpilot {
    tonic::include_proto!("netpilot.v1");
}
