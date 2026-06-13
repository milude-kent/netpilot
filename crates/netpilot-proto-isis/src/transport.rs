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
pub trait IsisTransport: Send {
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
