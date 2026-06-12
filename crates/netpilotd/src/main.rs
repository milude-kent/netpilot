use netpilotd::{api::build_router, state::AppState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = build_router(AppState::default());
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    let server = axum::serve(listener, app);

    // Graceful shutdown on SIGTERM/SIGINT
    let server = server.with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.ok();
        eprintln!("shutting down");
    });

    server.await?;
    Ok(())
}
