use crate::event::{ProtocolEvent, ProtocolStatus};
use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

#[async_trait]
pub trait ProtocolActor: Send + 'static {
    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError>;

    /// Set the event broadcast sender. Default no-op.
    fn set_event_tx(&mut self, _tx: tokio::sync::broadcast::Sender<ProtocolEvent>) {}
}

#[derive(Debug)]
// Variants carry full protocol configuration on Reload so the message
// crosses the actor boundary without back-references to a shared store.
// ProtocolMsg only travels through mpsc::Sender, so the per-variant size
// delta does not propagate to a hot stack path.
#[allow(clippy::large_enum_variant)]
pub enum ProtocolMsg {
    Reload {
        config: ProtocolConfig,
        scope: ReloadScope,
    },
    Enable,
    Disable,
    Restart,
    GracefulRestart,
    Shutdown,
    StatusQuery {
        reply: oneshot::Sender<ProtocolStatus>,
    },
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
