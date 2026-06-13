use crate::packet::EigrpPacket;
use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("socket error: {0}")]
    Socket(String),
}

#[async_trait]
pub trait EigrpTransport: Send {
    async fn send(&self, iface: &str, pkt: &EigrpPacket) -> Result<(), TransportError>;
    async fn recv(&mut self) -> Result<(String, EigrpPacket), TransportError>;
}

/// Loopback transport for testing
pub struct LoopbackTransport {
    queue: tokio::sync::mpsc::Receiver<(String, EigrpPacket)>,
    sender: tokio::sync::mpsc::Sender<(String, EigrpPacket)>,
}

impl LoopbackTransport {
    pub fn new_pair() -> (Self, tokio::sync::mpsc::Sender<(String, EigrpPacket)>) {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        (
            Self {
                queue: rx,
                sender: tx.clone(),
            },
            tx,
        )
    }
}

#[async_trait]
impl EigrpTransport for LoopbackTransport {
    async fn send(&self, iface: &str, pkt: &EigrpPacket) -> Result<(), TransportError> {
        let _ = self.sender.send((iface.to_string(), pkt.clone())).await;
        Ok(())
    }
    async fn recv(&mut self) -> Result<(String, EigrpPacket), TransportError> {
        self.queue
            .recv()
            .await
            .ok_or(TransportError::Socket("channel closed".into()))
    }
}
