use netpilot_config::ProtocolConfig;
use tokio::sync::{mpsc, oneshot};
use crate::actor::{ProtocolError, ProtocolMsg, ReloadScope};
use crate::event::ProtocolStatus;

#[derive(Clone, Debug)]
pub struct ProtocolHandle {
    name: String,
    tx: mpsc::Sender<ProtocolMsg>,
}

impl ProtocolHandle {
    pub fn new(name: String, tx: mpsc::Sender<ProtocolMsg>) -> Self {
        Self { name, tx }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn reload(
        &self,
        config: ProtocolConfig,
        scope: ReloadScope,
    ) -> Result<(), ProtocolError> {
        self.tx
            .send(ProtocolMsg::Reload { config, scope })
            .await
            .map_err(|_| ProtocolError::Stopped(self.name.clone(), "channel closed".into()))
    }

    pub async fn enable(&self) -> Result<(), ProtocolError> {
        self.tx
            .send(ProtocolMsg::Enable)
            .await
            .map_err(|_| ProtocolError::Stopped(self.name.clone(), "channel closed".into()))
    }

    pub async fn disable(&self) -> Result<(), ProtocolError> {
        self.tx
            .send(ProtocolMsg::Disable)
            .await
            .map_err(|_| ProtocolError::Stopped(self.name.clone(), "channel closed".into()))
    }

    pub async fn restart(&self) -> Result<(), ProtocolError> {
        self.tx
            .send(ProtocolMsg::Restart)
            .await
            .map_err(|_| ProtocolError::Stopped(self.name.clone(), "channel closed".into()))
    }

    pub async fn shutdown(&self) -> Result<(), ProtocolError> {
        self.tx
            .send(ProtocolMsg::Shutdown)
            .await
            .map_err(|_| ProtocolError::Stopped(self.name.clone(), "channel closed".into()))
    }

    pub async fn status(&self) -> Result<ProtocolStatus, ProtocolError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(ProtocolMsg::StatusQuery { reply })
            .await
            .map_err(|_| ProtocolError::Stopped(self.name.clone(), "channel closed".into()))?;
        rx.await
            .map_err(|_| ProtocolError::Stopped(self.name.clone(), "reply dropped".into()))
    }
}
