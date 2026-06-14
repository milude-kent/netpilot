use netpilot_config::ProtocolConfig;
use netpilot_protocol::ProtocolEvent;
use netpilotd::{api::build_router, state::AppState};
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Top-level CLI dispatch for `netpilotd`. Parsed before any
/// tokio runtime / config setup runs so subcommands like `token` can
/// exit cleanly without bringing up the daemon.
enum RootCommand {
    Run,
    Token { secret: String, ttl_secs: i64 },
    Help,
}

fn parse_root() -> RootCommand {
    let mut args = std::env::args().skip(1);
    let Some(sub) = args.next() else {
        return RootCommand::Run;
    };
    match sub.as_str() {
        "token" => {
            let mut secret: Option<String> = None;
            let mut ttl_secs: i64 = 3600;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--secret" | "-s" => secret = args.next(),
                    "--ttl-secs" | "-t" => {
                        if let Some(v) = args.next()
                            && let Ok(n) = v.parse()
                        {
                            ttl_secs = n;
                        }
                    }
                    "--help" | "-h" => return RootCommand::Help,
                    other => {
                        eprintln!("unknown token flag: {other}");
                        return RootCommand::Help;
                    }
                }
            }
            match secret {
                Some(s) => RootCommand::Token {
                    secret: s,
                    ttl_secs,
                },
                None => {
                    eprintln!("usage: netpilotd token --secret <secret> [--ttl-secs <n>]");
                    std::process::exit(2);
                }
            }
        }
        "--help" | "-h" | "help" => RootCommand::Help,
        _ => RootCommand::Run,
    }
}

fn print_help() {
    println!("netpilotd — NetPilot control plane daemon");
    println!();
    println!("USAGE:");
    println!("    netpilotd [SUBCOMMAND]");
    println!();
    println!("SUBCOMMANDS:");
    println!("    run                      Start the REST + gRPC servers (default).");
    println!("    token                    Generate a bearer token.");
    println!("        --secret, -s <s>     HMAC-SHA256 secret (required).");
    println!("        --ttl-secs, -t <n>   Token lifetime in seconds (default: 3600).");
    println!("    help                     Show this message.");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Handle subcommands that do not require a running daemon.
    match parse_root() {
        RootCommand::Help => {
            print_help();
            return Ok(());
        }
        RootCommand::Token { secret, ttl_secs } => {
            let token = netpilotd::auth_mw::generate_bearer_token(&secret, ttl_secs)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            println!("{token}");
            return Ok(());
        }
        RootCommand::Run => {}
    }

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
                    let actor =
                        netpilot_proto_isis::IsisActor::new(netpilot_proto_isis::IsisConfig {
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
                        });
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
                ProtocolConfig::Static { name, .. } => {
                    // Static routes are baked into the config snapshot at boot
                    // time; there is no long-running actor for them.
                    tracing::debug!(name = %name, "static routes — no actor needed");
                }
                ProtocolConfig::Ldp { name, .. } => {
                    let actor = netpilot_proto_ldp::LdpActor::new();
                    supervisor.spawn(name, protocol.clone(), actor);
                }
                ProtocolConfig::Pim { name, .. } => {
                    let actor = netpilot_proto_pim::PimActor::new();
                    supervisor.spawn(name, protocol.clone(), actor);
                }
                ProtocolConfig::Rip { name, .. } => {
                    let actor = netpilot_proto_rip::RipActor::new();
                    supervisor.spawn(name, protocol.clone(), actor);
                }
            }
        }
    }

    // Spawn RIB event processor with kernel sync
    let rib_state = app_state.rib.clone();
    let mut event_rx = app_state.supervisor.read().await.subscribe();
    tokio::spawn(async move {
        // Try to create kernel route client (will fail on macOS, OK on Linux)
        let kernel_client = netpilot_kernel::KernelRouteClient::new().await.ok();

        loop {
            match event_rx.recv().await {
                Ok(event) => {
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
                        ProtocolEvent::StateChange {
                            protocol_name: name,
                            new_state,
                            message: _,
                        } => {
                            rib.process_event(&event);
                            tracing::info!(
                                protocol = %name,
                                state = ?new_state,
                                "protocol state change"
                            );
                        }
                        ProtocolEvent::Error {
                            protocol_name: name,
                            message: err,
                        } => {
                            rib.process_event(&event);
                            tracing::error!(
                                protocol = %name,
                                error = %err,
                                "protocol error"
                            );
                        }
                        ProtocolEvent::Stats {
                            protocol_name: name,
                            stats,
                        } => {
                            tracing::debug!(
                                protocol = %name,
                                ?stats,
                                "protocol stats"
                            );
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    // Subscriber fell behind the broadcast ring; do not exit.
                    // The receiver position is automatically advanced past the
                    // dropped events on the next recv() call.
                    tracing::warn!(dropped = n, "supervisor event channel lagged");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    eprintln!("supervisor event channel closed; daemon exiting");
                    break;
                }
            }
        }
    });

    // Snapshot the auth config (if any) so we can decide whether to bind
    // a TLS listener, configure the gRPC server, and seed the bearer
    // secret into the running `AppState`.
    let auth_snapshot = {
        let cfg = app_state.config_store.read().await;
        cfg.running().auth.clone()
    };
    {
        let mut auth_state = app_state.auth.write().await;
        if let Some(a) = auth_snapshot.as_ref() {
            *auth_state = a.clone();
        }
    }

    // gRPC server (tonic)
    let grpc_state = {
        let event_tx = app_state.event_tx.clone();
        let mut gs =
            netpilot_grpc::GrpcAppState::with_event_tx(app_state.config_store.clone(), event_tx);
        if let Some(ref a) = auth_snapshot {
            gs = gs.with_auth(a.clone());
        }
        gs
    };
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

    // TLS for the REST listener is loaded when the auth config has both
    // cert and key paths. Because adding `axum-server` would mean a new
    // workspace dependency, the actual TLS handshake is a follow-up:
    // the cert/key are validated by `validate_tls_material` below so we
    // can fail fast at startup with a clear error if they are missing
    // or malformed. The plain HTTP listener continues to work.
    if let Some(ref a) = auth_snapshot
        && let (Some(cert), Some(key)) = (&a.tls_cert_path, &a.tls_key_path)
    {
        match netpilotd::auth_mw::validate_tls_material(cert, key) {
            Ok(()) => {
                tracing::warn!(
                    cert = %cert.display(),
                    key = %key.display(),
                    "TLS material is valid; the axum REST listener is still bound in plain HTTP — adding axum-server is tracked as an open item"
                );
            }
            Err(e) => {
                eprintln!("TLS material invalid: {e}");
                return Err(e.into());
            }
        }
    }

    let server = server.with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.ok();
        eprintln!("shutting down");
    });

    server.await?;
    grpc_task.abort();
    Ok(())
}
