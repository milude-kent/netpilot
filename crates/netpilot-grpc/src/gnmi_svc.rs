use crate::GrpcAppState;
use crate::gnmi::{
    CapabilityRequest, CapabilityResponse, GetRequest, GetResponse, Path, PathValue, SetRequest,
    SetResponse, SubscribeRequest, SubscribeResponse,
    g_nmi_server::{GNmi, GNmiServer},
    subscribe_request, subscribe_response, subscription_list,
};
use crate::path_resolver;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

pub struct GnmiService {
    state: GrpcAppState,
}

impl GnmiService {
    pub fn new(state: GrpcAppState) -> Self {
        Self { state }
    }
    pub fn into_gnmi_server(self) -> GNmiServer<Self> {
        GNmiServer::new(self)
    }
}

#[tonic::async_trait]
impl GNmi for GnmiService {
    async fn capabilities(
        &self,
        _: Request<CapabilityRequest>,
    ) -> Result<Response<CapabilityResponse>, Status> {
        Ok(Response::new(CapabilityResponse {
            supported_models: vec!["netpilot-v1".into()],
            supported_encodings: vec!["JSON".into()],
            version: "0.1.0".into(),
        }))
    }

    async fn get(&self, req: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let paths = &req.get_ref().path;
        let values: Vec<PathValue> = paths
            .iter()
            .filter_map(|p| path_resolver::resolve(&self.state, p))
            .collect();
        Ok(Response::new(GetResponse { value: values }))
    }

    async fn set(&self, req: Request<SetRequest>) -> Result<Response<SetResponse>, Status> {
        let req = req.into_inner();
        let config_path = Path {
            elem: vec!["netpilot".into(), "config".into(), "candidate".into()],
        };

        for pv in &req.replace {
            if pv
                .path
                .as_ref()
                .map(|p| p.elem == config_path.elem)
                .unwrap_or(false)
            {
                let config: netpilot_config::RoutePlaneConfig = serde_json::from_slice(&pv.value)
                    .map_err(|e| {
                    Status::invalid_argument(format!("invalid config JSON: {e}"))
                })?;
                netpilot_config::validation::validate_config(&config)
                    .map_err(|e| Status::invalid_argument(format!("validation error: {e}")))?;
                self.state
                    .config_store
                    .write()
                    .await
                    .replace_candidate(config)
                    .map_err(|e| Status::internal(format!("config store error: {e}")))?;
                return Ok(Response::new(SetResponse {
                    value: vec![],
                    message: "candidate config updated".into(),
                }));
            }
        }

        if !req.update.is_empty() || !req.delete.is_empty() {
            return Err(Status::unimplemented("gNMI update/delete not supported"));
        }

        Ok(Response::new(SetResponse {
            value: vec![],
            message: "no changes applied".into(),
        }))
    }

    type SubscribeStream = ReceiverStream<Result<SubscribeResponse, Status>>;

    async fn subscribe(
        &self,
        req: Request<Streaming<SubscribeRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let mut stream = req.into_inner();
        let state = self.state.clone();
        let (tx, rx) = mpsc::channel(16);

        tokio::spawn(async move {
            // The mode is carried in each SubscribeRequest. For ONCE and
            // STREAM we expect a single request that drives the lifetime
            // of the session; for POLL the client resends Subscribe
            // messages and we re-snapshot on each one.
            while let Ok(Some(msg)) = stream.message().await {
                let list = match msg.request.as_ref() {
                    Some(subscribe_request::Request::Subscribe(list)) => list,
                    None => {
                        let _ = tx
                            .send(Err(Status::invalid_argument("missing Subscribe request")))
                            .await;
                        return;
                    }
                };

                let mode = match subscription_list::Mode::try_from(list.mode) {
                    Ok(m) => m,
                    Err(_) => {
                        let _ = tx
                            .send(Err(Status::invalid_argument("unknown subscribe mode")))
                            .await;
                        return;
                    }
                };

                match mode {
                    subscription_list::Mode::Once => {
                        if !emit_snapshot_blocking(&tx, &state, list).await {
                            return;
                        }
                        // Once mode completes; close the stream.
                        return;
                    }
                    subscription_list::Mode::Stream => {
                        if !emit_snapshot_blocking(&tx, &state, list).await {
                            return;
                        }
                        run_stream_mode(&tx, &state, &mut stream).await;
                        return;
                    }
                    subscription_list::Mode::Poll => {
                        if !emit_snapshot_blocking(&tx, &state, list).await {
                            return;
                        }
                        // Stay in POLL mode: keep reading Subscribe
                        // messages and re-snapshot on each one. The
                        // client drives the polling cadence.
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Emit one update per path in the subscription list, then a sync
/// response. Returns `false` if the client has dropped the stream.
async fn emit_snapshot_blocking(
    tx: &mpsc::Sender<Result<SubscribeResponse, Status>>,
    state: &GrpcAppState,
    list: &crate::gnmi::SubscriptionList,
) -> bool {
    for sub in &list.subscription {
        if let Some(ref path) = sub.path
            && let Some(pv) = path_resolver::resolve(state, path)
            && tx
                .send(Ok(SubscribeResponse {
                    response: Some(subscribe_response::Response::Update(pv)),
                }))
                .await
                .is_err()
        {
            return false;
        }
    }
    tx.send(Ok(SubscribeResponse {
        response: Some(subscribe_response::Response::SyncResponse(true)),
    }))
    .await
    .is_ok()
}

/// STREAM mode: forward every event published on the supervisor's
/// broadcast channel until either side disconnects. Each event is sent
/// as a `PathValue` update under the synthetic `netpilot/events` path.
async fn run_stream_mode(
    tx: &mpsc::Sender<Result<SubscribeResponse, Status>>,
    state: &GrpcAppState,
    stream: &mut Streaming<SubscribeRequest>,
) {
    let Some(event_tx) = state.event_tx.clone() else {
        // No broadcast channel wired up. Stay alive until the client
        // disconnects so we don't close the stream prematurely.
        if stream.message().await.is_ok() {
            tracing::debug!("Subscribe (STREAM) closing: no event channel wired");
        }
        return;
    };

    let mut event_rx = event_tx.subscribe();
    loop {
        tokio::select! {
            biased;
            client_msg = stream.message() => {
                match client_msg {
                    Ok(Some(_)) => {
                        // Subsequent Subscribe messages on a STREAM
                        // session are ignored. We keep the channel
                        // open for the rest of the event stream.
                    }
                    Ok(None) => return,
                    Err(e) => {
                        tracing::warn!(error = %e, "Subscribe stream client error");
                        return;
                    }
                }
            }
            event = event_rx.recv() => {
                match event {
                    Ok(ev) => {
                        let json = match serde_json::to_vec(&ev) {
                            Ok(b) => b,
                            Err(e) => {
                                tracing::warn!(error = %e, "serialize ProtocolEvent for Subscribe");
                                continue;
                            }
                        };
                        let pv = PathValue {
                            path: Some(Path {
                                elem: vec!["netpilot".into(), "events".into()],
                            }),
                            value: json,
                        };
                        if tx
                            .send(Ok(SubscribeResponse {
                                response: Some(subscribe_response::Response::Update(pv)),
                            }))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "Subscribe broadcast lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return;
                    }
                }
            }
        }
    }
}
