use netpilot_config::ProtocolConfig;
use netpilot_protocol::ProtocolEvent;
use netpilotd::{api::build_router, state::AppState};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_state = AppState::default();

    // Spawn protocol actors from running config
    {
        let config_store = app_state.config_store.read().await;
        let running = config_store.running();
        let mut supervisor = app_state.supervisor.write().await;

        for protocol in &running.protocols {
            match protocol {
                ProtocolConfig::Bgp { name, .. } => {
                    let actor = netpilot_proto_bgp::actor::BgpActor::new();
                    supervisor.spawn(name, protocol.clone(), actor);
                }
                ProtocolConfig::Ospf { name, .. } => {
                    let actor = netpilot_proto_ospf::OspfActor::new();
                    supervisor.spawn(name, protocol.clone(), actor);
                }
                ProtocolConfig::Isis { name, .. } => {
                    let actor = netpilot_proto_isis::IsisActor::new(
                        netpilot_proto_isis::IsisConfig {
                            name: name.clone(),
                            table: "master".into(),
                            area_addresses: vec![],
                            system_id: String::new(),
                            levels: vec![],
                            interfaces: vec![],
                            sr_enabled: None,
                            limits: None,
                            import_keep_filtered: None,
                            rpki_reload: None,
                            passwords: None,
                            password: None,
                            tx_class: None,
                            tx_priority: None,
                            description: None,
                        },
                    );
                    supervisor.spawn(name, protocol.clone(), actor);
                }
                ProtocolConfig::Eigrp { name, .. } => {
                    let config = netpilot_proto_eigrp::EigrpConfig {
                        name: name.clone(),
                        table: "master".into(),
                        autonomous_system: 0,
                        router_id: String::new(),
                        interfaces: vec![],
                        k_values: None,
                        maximum_paths: None,
                        variance: None,
                        limits: None,
                        import_keep_filtered: None,
                        rpki_reload: None,
                        passwords: None,
                        password: None,
                        tx_class: None,
                        tx_priority: None,
                        description: None,
                    };
                    let actor = netpilot_proto_eigrp::actor::EigrpActor::new(config);
                    supervisor.spawn(name, protocol.clone(), actor);
                }
                _ => {} // LDP, PIM, RIP, etc. — spawn stubs if actor exists
            }
        }
    }

    // Spawn RIB event processor with kernel sync
    let rib_state = app_state.rib.clone();
    let mut event_rx = app_state.supervisor.read().await.subscribe();
    tokio::spawn(async move {
        // Try to create kernel route client (will fail on macOS, OK on Linux)
        let kernel_client = netpilot_kernel::KernelRouteClient::new().await.ok();

        while let Ok(event) = event_rx.recv().await {
            let mut rib = rib_state.write().await;
            match &event {
                ProtocolEvent::RouteAnnounce {
                    table: _,
                    prefix,
                    next_hop,
                    preference: _,
                    attributes: _,
                } => {
                    rib.process_event(&event);

                    // Install into kernel FIB if client is available
                    if let Some(ref kc) = kernel_client {
                        let route = netpilot_kernel::KernelRoute::new(prefix)
                            .with_next_hop(next_hop)
                            .with_table(254)
                            .with_protocol(netpilot_kernel::RouteProtocol::Bgp);
                        if let Err(e) = kc.add(&route).await {
                            eprintln!("kernel route add failed: {}", e);
                        }
                    }
                }
                ProtocolEvent::RouteWithdraw { table: _, prefix } => {
                    rib.process_event(&event);
                    if let Some(ref kc) = kernel_client {
                        let route = netpilot_kernel::KernelRoute::new(prefix);
                        if let Err(e) = kc.delete(&route).await {
                            eprintln!("kernel route delete failed: {}", e);
                        }
                    }
                }
                _ => {
                    rib.process_event(&event);
                }
            }
        }
    });

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
