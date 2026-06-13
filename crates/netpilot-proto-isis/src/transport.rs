use async_trait::async_trait;
use crate::packet::IsisPacket;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("socket error: {0}")]
    Socket(String),
    #[error("interface not found: {0}")]
    InterfaceNotFound(String),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Abstract IS-IS packet transport. Protocol logic uses this trait
/// without knowing about raw sockets.
#[async_trait]
pub trait IsisTransport: Send + Sync {
    /// Send an IS-IS packet on a specific interface.
    async fn send(&self, iface: &str, pkt: &IsisPacket) -> Result<(), TransportError>;

    /// Receive the next IS-IS packet, returning the interface it arrived on.
    async fn recv(&mut self) -> Result<(String, IsisPacket), TransportError>;
}

/// Platform transport that returns empty/errors on non-Linux or when
/// no raw socket is configured. A full implementation would use socket2
/// to open AF_PACKET sockets and send/receive IS-IS frames.
pub struct RawSocketTransport {
    // socket2::Socket would go here on Linux
    #[allow(dead_code)]
    dummy: bool,
}

impl RawSocketTransport {
    pub fn new() -> Result<Self, TransportError> {
        Ok(Self { dummy: true })
    }
}

#[async_trait]
impl IsisTransport for RawSocketTransport {
    async fn send(&self, _iface: &str, _pkt: &IsisPacket) -> Result<(), TransportError> {
        // Placeholder: actual raw socket send on Linux
        Err(TransportError::Socket("raw socket not implemented".into()))
    }

    async fn recv(&mut self) -> Result<(String, IsisPacket), TransportError> {
        // Placeholder: return pending forever until implemented
        std::future::pending::<()>().await;
        unreachable!()
    }
}

/// Real transport implementation using a channel-based loopback for testing.
/// In production, this would use socket2 raw sockets.
pub struct LoopbackTransport {
    queue: tokio::sync::mpsc::Receiver<(String, IsisPacket)>,
    sender: tokio::sync::mpsc::Sender<(String, IsisPacket)>,
}

impl LoopbackTransport {
    pub fn new_pair() -> (Self, tokio::sync::mpsc::Sender<(String, IsisPacket)>) {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        (Self { queue: rx, sender: tx.clone() }, tx)
    }
}

#[async_trait]
impl IsisTransport for LoopbackTransport {
    async fn send(&self, iface: &str, pkt: &IsisPacket) -> Result<(), TransportError> {
        // In real implementation: send via raw socket
        // For loopback: put into the channel for recv
        let _ = self.sender.send((iface.to_string(), pkt.clone())).await;
        Ok(())
    }

    async fn recv(&mut self) -> Result<(String, IsisPacket), TransportError> {
        self.queue.recv().await
            .ok_or(TransportError::Socket("channel closed".into()))
    }
}
