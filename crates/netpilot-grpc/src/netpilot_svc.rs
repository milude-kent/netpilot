use crate::netpilot::{
    config_server::{Config, ConfigServer},
    health_server::{Health, HealthServer},
    CommitConfigRequest, CommitConfigResponse, GetRunningRequest, GetRunningResponse,
    HealthCheckRequest, HealthCheckResponse, PutCandidateRequest, PutCandidateResponse,
    RollbackRequest, RollbackResponse,
};
use crate::GrpcAppState;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub struct ConfigService { state: GrpcAppState }
impl ConfigService {
    pub fn new(state: GrpcAppState) -> Self { Self { state } }
    pub fn into_config_server(self) -> ConfigServer<Self> { ConfigServer::new(self) }
}

#[tonic::async_trait]
impl Config for ConfigService {
    async fn get_running(&self, _: Request<GetRunningRequest>) -> Result<Response<GetRunningResponse>, Status> {
        let store = self.state.config_store.read().await;
        let config = store.running();
        let json = serde_json::to_vec(config).map_err(|e| Status::internal(format!("serialize: {e}")))?;
        let revision_id = store.revisions().last().map(|r| r.id as u64).unwrap_or(0);
        Ok(Response::new(GetRunningResponse { config_json: json, revision_id }))
    }

    async fn put_candidate(&self, req: Request<PutCandidateRequest>) -> Result<Response<PutCandidateResponse>, Status> {
        let config: netpilot_config::RoutePlaneConfig = serde_json::from_slice(&req.get_ref().config_json)
            .map_err(|e| Status::invalid_argument(format!("invalid JSON: {e}")))?;
        netpilot_config::validation::validate_config(&config)
            .map_err(|e| Status::invalid_argument(format!("validation: {e}")))?;
        self.state.config_store.write().await.replace_candidate(config)
            .map_err(|e| Status::internal(format!("store error: {e}")))?;
        Ok(Response::new(PutCandidateResponse { status: "candidate updated".into() }))
    }

    async fn commit_config(&self, req: Request<CommitConfigRequest>) -> Result<Response<CommitConfigResponse>, Status> {
        let req = req.into_inner();
        let revision = self.state.config_store.write().await
            .commit(netpilot_config::CommitRequest { author: req.author, note: req.note })
            .map_err(|e| Status::internal(format!("commit error: {e}")))?;
        Ok(Response::new(CommitConfigResponse { revision_id: revision.id as u64, message: format!("committed revision {}", revision.id) }))
    }

    async fn rollback(&self, req: Request<RollbackRequest>) -> Result<Response<RollbackResponse>, Status> {
        let req = req.into_inner();
        let revision = self.state.config_store.write().await
            .rollback(netpilot_config::RollbackRequest { revision_id: req.revision_id, author: req.author, note: req.note })
            .map_err(|e| Status::internal(format!("rollback error: {e}")))?;
        Ok(Response::new(RollbackResponse { revision_id: revision.id as u64, message: format!("rolled back to revision {}", revision.id) }))
    }
}

pub struct HealthService { state: GrpcAppState }
impl HealthService {
    pub fn new(state: GrpcAppState) -> Self { Self { state } }
    pub fn into_health_server(self) -> HealthServer<Self> { HealthServer::new(self) }
}

#[tonic::async_trait]
impl Health for HealthService {
    async fn check(&self, _: Request<HealthCheckRequest>) -> Result<Response<HealthCheckResponse>, Status> {
        let store = self.state.config_store.read().await;
        let revision = store.revisions().last().map(|r| r.id as u32).unwrap_or(0);
        Ok(Response::new(HealthCheckResponse { status: "SERVING".into(), uptime_secs: "0".into(), config_revision: revision }))
    }

    type WatchStream = ReceiverStream<Result<HealthCheckResponse, Status>>;

    async fn watch(&self, _: Request<HealthCheckRequest>) -> Result<Response<Self::WatchStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                let store = state.config_store.read().await;
                let revision = store.revisions().last().map(|r| r.id).unwrap_or(0);
                let _ = tx.send(Ok(HealthCheckResponse { status: "SERVING".into(), uptime_secs: "0".into(), config_revision: revision as u32 })).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
