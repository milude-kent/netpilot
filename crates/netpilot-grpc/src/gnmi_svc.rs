use crate::gnmi::{
    g_nmi_server::{GNmi, GNmiServer},
    subscribe_request, subscribe_response, subscription_list, CapabilityRequest,
    CapabilityResponse, GetRequest, GetResponse, Path, PathValue, SetRequest, SetResponse,
    SubscribeRequest, SubscribeResponse,
};
use crate::path_resolver;
use crate::GrpcAppState;
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

    async fn get(
        &self,
        req: Request<GetRequest>,
    ) -> Result<Response<GetResponse>, Status> {
        let paths = &req.get_ref().path;
        let values: Vec<PathValue> = paths
            .iter()
            .filter_map(|p| path_resolver::resolve(&self.state, p))
            .collect();
        Ok(Response::new(GetResponse { value: values }))
    }

    async fn set(
        &self,
        req: Request<SetRequest>,
    ) -> Result<Response<SetResponse>, Status> {
        let req = req.into_inner();
        let config_path = Path {
            elem: vec!["netpilot".into(), "config".into(), "candidate".into()],
        };

        for pv in &req.replace {
            if pv.path.as_ref().map(|p| p.elem == config_path.elem).unwrap_or(false) {
                let config: netpilot_config::RoutePlaneConfig =
                    serde_json::from_slice(&pv.value).map_err(|e| {
                        Status::invalid_argument(format!("invalid config JSON: {e}"))
                    })?;
                netpilot_config::validation::validate_config(&config).map_err(|e| {
                    Status::invalid_argument(format!("validation error: {e}"))
                })?;
                self.state.config_store.write().await.replace_candidate(config).map_err(|e| {
                    Status::internal(format!("config store error: {e}"))
                })?;
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
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        tokio::spawn(async move {
            while let Ok(Some(msg)) = stream.message().await {
                if let Some(subscribe_request::Request::Subscribe(list)) = msg.request {
                    match subscription_list::Mode::try_from(list.mode) {
                        Ok(subscription_list::Mode::Once) => {
                            for sub in &list.subscription {
                                if let Some(ref path) = sub.path {
                                    if let Some(pv) = path_resolver::resolve(&state, path) {
                                        let _ = tx.send(Ok(SubscribeResponse {
                                            response: Some(
                                                subscribe_response::Response::Update(pv),
                                            ),
                                        })).await;
                                    }
                                }
                            }
                            let _ = tx.send(Ok(SubscribeResponse {
                                response: Some(
                                    subscribe_response::Response::SyncResponse(true),
                                ),
                            })).await;
                        }
                        Ok(subscription_list::Mode::Stream) => {
                            for sub in &list.subscription {
                                if let Some(ref path) = sub.path {
                                    if let Some(pv) = path_resolver::resolve(&state, path) {
                                        let _ = tx.send(Ok(SubscribeResponse {
                                            response: Some(
                                                subscribe_response::Response::Update(pv),
                                            ),
                                        })).await;
                                    }
                                }
                            }
                            let _ = tx.send(Ok(SubscribeResponse {
                                response: Some(
                                    subscribe_response::Response::SyncResponse(true),
                                ),
                            })).await;
                        }
                        _ => {
                            let _ = tx.send(Err(Status::unimplemented("POLL not supported"))).await;
                        }
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
