use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use crate::event::ProtocolStatus;

#[async_trait]
pub trait ProtocolActor: Send + 'static {
    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError>;
}

#[derive(Debug)]
pub enum ProtocolMsg {
    Reload { config: ProtocolConfig, scope: ReloadScope },
    Enable,
    Disable,
    Restart,
    GracefulRestart,
    Shutdown,
    StatusQuery { reply: oneshot::Sender<ProtocolStatus> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReloadScope {
    Full,
    Import,
    Export,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("protocol '{0}' stopped: {1}")]
    Stopped(String, String),
    #[error("configuration error: {0}")]
    Config(String),
}
