pub mod gnmi_svc;
pub mod netpilot_svc;
pub mod path_resolver;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tonic::service::interceptor;

/// State shared with gRPC services. Holds the config store and (when
/// wired up by `netpilotd`) the supervisor's event broadcast sender so
/// `Subscribe` in `Stream` mode can fan out live protocol events.
#[derive(Clone)]
pub struct GrpcAppState {
    pub config_store: Arc<RwLock<netpilot_config::ConfigStore>>,
    /// Optional broadcast sender for `ProtocolEvent`s. When `None`,
    /// `Subscribe` in `Stream` mode emits a single sync and closes; when
    /// `Some`, it subscribes to the channel and forwards events as
    /// `Update` messages until either side disconnects.
    pub event_tx: Option<Arc<broadcast::Sender<netpilot_protocol::ProtocolEvent>>>,
    /// Optional auth / TLS configuration. When `Some`, an
    /// [`AuthInterceptor`] is installed on every service that rejects
    /// requests missing a valid `authorization: Bearer <token>` header.
    /// When `None`, the server is unauthenticated and uses plain
    /// HTTP/2 (no TLS) — this is the trusted-network default and
    /// matches the pre-C1 behavior.
    pub auth: Option<Arc<netpilot_config::AuthConfig>>,
}

impl GrpcAppState {
    pub fn new(config_store: Arc<RwLock<netpilot_config::ConfigStore>>) -> Self {
        Self {
            config_store,
            event_tx: None,
            auth: None,
        }
    }

    /// Build a `GrpcAppState` with an event broadcast sender attached.
    /// `netpilotd` uses this constructor so the gNMI `Subscribe` stream
    /// can serve live protocol events.
    pub fn with_event_tx(
        config_store: Arc<RwLock<netpilot_config::ConfigStore>>,
        event_tx: Arc<broadcast::Sender<netpilot_protocol::ProtocolEvent>>,
    ) -> Self {
        Self {
            config_store,
            event_tx: Some(event_tx),
            auth: None,
        }
    }

    /// Attach an `AuthConfig` to the gRPC state. The interceptor is
    /// applied automatically in [`serve`] below.
    pub fn with_auth(mut self, auth: netpilot_config::AuthConfig) -> Self {
        self.auth = Some(Arc::new(auth));
        self
    }
}

/// Tonic request interceptor that requires every RPC to carry a valid
/// `authorization: Bearer <token>` header when the server is configured
/// with an `AuthConfig`. When the request is unauthenticated or carries
/// an invalid token, the request is rejected with
/// `tonic::Status::unauthenticated` before the handler is invoked.
#[derive(Clone)]
pub struct AuthInterceptor {
    auth: Arc<netpilot_config::AuthConfig>,
}

impl AuthInterceptor {
    pub fn new(auth: Arc<netpilot_config::AuthConfig>) -> Self {
        Self { auth }
    }
}

/// Pass-through interceptor used when no auth is configured. Keeps the
/// `Server<L>` type the same regardless of whether auth is enabled, so
/// the rest of the server setup does not need a conditional.
#[derive(Clone, Copy, Debug)]
pub struct NoopInterceptor;

impl tonic::service::Interceptor for NoopInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        Ok(request)
    }
}

/// Internal helper: either a real auth interceptor or a pass-through.
/// Used so the gRPC server's generic `Server<L>` type is uniform.
#[derive(Clone)]
pub enum EitherInterceptor {
    Auth(AuthInterceptor),
    Noop,
}

impl tonic::service::Interceptor for EitherInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        match self {
            EitherInterceptor::Auth(a) => a.call(request),
            EitherInterceptor::Noop => Ok(request),
        }
    }
}

/// Validate a `<exp>.<sig_hex>` bearer token. Mirrors the
/// `netpilotd::auth_mw::validate_bearer` logic and is duplicated here
/// to avoid a workspace-level dependency on `netpilotd`. The token
/// format is:
///   * `<exp>` is a unix-epoch expiry (decimal).
///   * `<sig_hex>` is the hex-encoded HMAC-SHA256 of `<exp>` using the
///     configured `bearer_secret`.
pub fn validate_bearer_token(token: &str, auth: &netpilot_config::AuthConfig) -> bool {
    use netpilot_auth::{AuthAlgorithm, verify_auth};
    let secret = match auth.bearer_secret.as_deref() {
        Some(s) => s,
        None => return false,
    };
    let mut parts = token.splitn(2, '.');
    let exp_str = match parts.next() {
        Some(s) => s,
        None => return false,
    };
    let sig_hex = match parts.next() {
        Some(s) => s,
        None => return false,
    };
    let exp: i64 = match exp_str.parse() {
        Ok(n) => n,
        Err(_) => return false,
    };
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    if exp < now {
        return false;
    }
    let provided_sig = match hex_decode(sig_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    verify_auth(
        &AuthAlgorithm::HmacSha256,
        secret.as_bytes(),
        exp_str.as_bytes(),
        &provided_sig,
    )
    .unwrap_or(false)
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if !s.len().is_multiple_of(2) {
        return Err(());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, ()> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(()),
    }
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        let header = request
            .metadata()
            .get("authorization")
            .and_then(|h| h.to_str().ok());
        let token = header.and_then(|h| h.strip_prefix("Bearer ")).unwrap_or("");
        if !validate_bearer_token(token, &self.auth) {
            return Err(tonic::Status::unauthenticated(
                "missing or invalid bearer token",
            ));
        }
        Ok(request)
    }
}

/// Build the tonic `Interceptor` to wrap each gRPC service with.
/// Returns `None` when no auth is configured.
pub fn auth_interceptor(
    auth: Option<&Arc<netpilot_config::AuthConfig>>,
) -> Option<AuthInterceptor> {
    auth.map(|a| AuthInterceptor::new(a.clone()))
}

/// Start the gRPC server. Returns a future that resolves when the server stops.
pub async fn serve(
    addr: SocketAddr,
    state: GrpcAppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tonic::transport::{Server, ServerTlsConfig};

    // Snapshot auth + TLS config before moving `state` into the
    // service constructors.
    let auth_snapshot = state.auth.clone();
    let tls_paths =
        auth_snapshot
            .as_ref()
            .and_then(|a| match (&a.tls_cert_path, &a.tls_key_path) {
                (Some(c), Some(k)) => Some((c.clone(), k.clone())),
                _ => None,
            });

    let gnmi_svc = gnmi_svc::GnmiService::new(state.clone());
    let config_svc = netpilot_svc::ConfigService::new(state.clone());
    let health_svc = netpilot_svc::HealthService::new(state);

    // When auth is enabled, apply the interceptor as a server-level
    // layer so every RPC goes through it. (The health service is
    // intentionally NOT exempt — clients should be able to authenticate
    // for liveness probes too. Operators that want unauthenticated
    // health checks can simply not configure bearer auth.)
    //
    // We always go through the `layer()` codepath so the builder type
    // is the same regardless of whether auth is configured. The
    // no-auth branch uses the [`NoopInterceptor`] pass-through below.
    let layer: EitherInterceptor = match auth_interceptor(auth_snapshot.as_ref()) {
        Some(intc) => EitherInterceptor::Auth(intc),
        None => EitherInterceptor::Noop,
    };
    let mut builder = Server::builder().layer(interceptor::interceptor(layer));

    // TLS for the gRPC server is configured when the auth config has
    // both cert and key paths.
    if let Some((cert, key)) = tls_paths {
        let identity = tonic::transport::Identity::from_pem(
            std::fs::read(&cert).map_err(|e| format!("read gRPC cert: {e}"))?,
            std::fs::read(&key).map_err(|e| format!("read gRPC key: {e}"))?,
        );
        let tls = ServerTlsConfig::new().identity(identity);
        builder = builder
            .tls_config(tls)
            .map_err(|e| format!("configure gRPC TLS: {e}"))?;
    }

    builder
        .add_service(gnmi_svc.into_gnmi_server())
        .add_service(config_svc.into_config_server())
        .add_service(health_svc.into_health_server())
        .serve(addr)
        .await?;

    Ok(())
}

/// Proto-generated modules. These are at the crate root because tonic::include_proto!
/// generates nested modules from dotted names.
pub mod gnmi {
    tonic::include_proto!("gnmi.v1");
}
pub mod netpilot {
    tonic::include_proto!("netpilot.v1");
}

#[cfg(test)]
mod tests {
    use super::*;
    use netpilot_config::AuthConfig;
    fn auth_with(secret: &str) -> AuthConfig {
        AuthConfig {
            bearer_secret: Some(secret.into()),
            ..AuthConfig::default()
        }
    }

    fn hex_encode(bytes: &[u8]) -> String {
        const HEX: &[u8] = b"0123456789abcdef";
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push(HEX[(b >> 4) as usize] as char);
            s.push(HEX[(b & 0x0f) as usize] as char);
        }
        s
    }

    #[test]
    fn validate_bearer_token_accepts_valid() {
        let auth = auth_with("k");
        let exp = time::OffsetDateTime::now_utc().unix_timestamp() + 60;
        let exp_str = exp.to_string();
        let sig = netpilot_auth::compute_auth(
            &netpilot_config::AuthAlgorithm::HmacSha256,
            b"k",
            exp_str.as_bytes(),
        )
        .unwrap();
        let token = format!("{}.{}", exp_str, hex_encode(&sig));
        assert!(validate_bearer_token(&token, &auth));
    }

    #[test]
    fn validate_bearer_token_rejects_expired() {
        let auth = auth_with("k");
        let exp = time::OffsetDateTime::now_utc().unix_timestamp() - 1;
        let exp_str = exp.to_string();
        let sig = netpilot_auth::compute_auth(
            &netpilot_config::AuthAlgorithm::HmacSha256,
            b"k",
            exp_str.as_bytes(),
        )
        .unwrap();
        let token = format!("{}.{}", exp_str, hex_encode(&sig));
        assert!(!validate_bearer_token(&token, &auth));
    }

    #[test]
    fn validate_bearer_token_rejects_wrong_secret() {
        let auth = auth_with("right");
        let exp = time::OffsetDateTime::now_utc().unix_timestamp() + 60;
        let exp_str = exp.to_string();
        let sig = netpilot_auth::compute_auth(
            &netpilot_config::AuthAlgorithm::HmacSha256,
            b"wrong",
            exp_str.as_bytes(),
        )
        .unwrap();
        let token = format!("{}.{}", exp_str, hex_encode(&sig));
        assert!(!validate_bearer_token(&token, &auth));
    }

    #[test]
    fn validate_bearer_token_rejects_garbage() {
        let auth = auth_with("k");
        assert!(!validate_bearer_token("", &auth));
        assert!(!validate_bearer_token("not-a-token", &auth));
        assert!(!validate_bearer_token("abc", &auth));
        assert!(!validate_bearer_token("abc.def", &auth));
    }

    #[test]
    fn interceptor_rejects_without_token() {
        let auth = Arc::new(auth_with("k"));
        let mut interceptor = AuthInterceptor::new(auth);
        let req = tonic::Request::new(());
        let result = tonic::service::Interceptor::call(&mut interceptor, req);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn interceptor_accepts_valid_token() {
        let auth = Arc::new(auth_with("k"));
        let exp = time::OffsetDateTime::now_utc().unix_timestamp() + 60;
        let exp_str = exp.to_string();
        let sig = netpilot_auth::compute_auth(
            &netpilot_config::AuthAlgorithm::HmacSha256,
            b"k",
            exp_str.as_bytes(),
        )
        .unwrap();
        let token = format!("Bearer {}.{}", exp_str, hex_encode(&sig));

        let mut interceptor = AuthInterceptor::new(auth);
        let mut req = tonic::Request::new(());
        req.metadata_mut()
            .insert("authorization", token.parse().unwrap());
        let result = tonic::service::Interceptor::call(&mut interceptor, req);
        assert!(result.is_ok());
    }
}
