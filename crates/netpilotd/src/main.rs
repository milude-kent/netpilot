use netpilotd::{api::build_router, state::AppState};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_state = AppState::default();

    // gRPC server (tonic)
    let grpc_state = netpilot_grpc::GrpcAppState::new(app_state.config_store.clone());
    let grpc_addr: SocketAddr = "127.0.0.1:50051".parse()?;
    let grpc_task = tokio::spawn(async move {
        if let Err(e) = netpilot_grpc::serve(grpc_addr, grpc_state).await {
            eprintln!("gRPC server error: {e}");
        }
    });

    // REST API (axum)
    let app = build_router(app_state);
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    let server = axum::serve(listener, app);

    let server = server.with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.ok();
        eprintln!("shutting down");
    });

    server.await?;
    grpc_task.abort();
    Ok(())
}
