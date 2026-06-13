use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("netlink error: {0}")]
    Netlink(String),

    #[error("interface not found: {0}")]
    InterfaceNotFound(String),

    #[error("route operation failed: {0}")]
    RouteOperationFailed(String),

    #[error("unsupported platform")]
    UnsupportedPlatform,
}
