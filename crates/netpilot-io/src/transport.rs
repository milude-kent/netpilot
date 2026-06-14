use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("socket error: {0}")]
    Socket(String),
    #[error("unsupported platform")]
    UnsupportedPlatform,
}

/// Raw socket abstraction for OSPF (IP proto 89) and IS-IS (link-level).
pub struct RawSocket {
    #[cfg(target_os = "linux")]
    socket: Option<socket2::Socket>,
}

impl RawSocket {
    #[allow(unreachable_code)]
    pub fn new() -> Result<Self, TransportError> {
        #[cfg(target_os = "linux")]
        {
            use socket2::{Domain, Protocol, Type};
            let socket = socket2::Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::from(89)))
                .map_err(|e| TransportError::Socket(e.to_string()))?;
            socket
                .set_nonblocking(true)
                .map_err(|e| TransportError::Socket(e.to_string()))?;
            return Ok(Self {
                socket: Some(socket),
            });
        }
        Ok(Self {
            #[cfg(target_os = "linux")]
            socket: None,
        })
    }

    /// Send raw bytes on the socket.
    pub async fn send(&self, _data: &[u8]) -> Result<usize, TransportError> {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref socket) = self.socket {
                // Real implementation would use socket.send_to() with the
                // destination address. For now, return 0 as a placeholder
                // (the socket exists but the send logic is not yet wired).
                return Ok(0);
            }
        }
        Err(TransportError::UnsupportedPlatform)
    }

    /// Receive raw bytes from the socket.
    pub async fn recv(&self, _buf: &mut [u8]) -> Result<usize, TransportError> {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref _socket) = self.socket {
                // Real implementation would use socket.recv_from().
                return Ok(0);
            }
        }
        Err(TransportError::UnsupportedPlatform)
    }
}

/// High-level transport trait for OSPF protocol.
#[async_trait]
pub trait OspfTransport: Send + Sync {
    async fn send_hello(&self, iface: &str) -> Result<(), TransportError>;
    async fn recv_packet(&mut self) -> Result<(String, Vec<u8>), TransportError>;
}

/// High-level transport trait for IS-IS protocol.
#[async_trait]
pub trait IsisTransport: Send + Sync {
    async fn send_pdu(&self, iface: &str, data: &[u8]) -> Result<(), TransportError>;
    async fn recv_pdu(&mut self) -> Result<(String, Vec<u8>), TransportError>;
}
