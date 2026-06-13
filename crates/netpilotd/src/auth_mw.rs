//! Axum middleware: Bearer-token authentication.
//!
//! Tokens have the form `<exp_unix>.<hex_hmac>` where `<hex_hmac>` is the
//! hex-encoded HMAC-SHA256 of `<exp_unix>` using [`AuthConfig::bearer_secret`]
//! as the key. Tokens whose `exp_unix` is in the past are rejected. The
//! allowlist in [`AuthConfig::unauthed_paths`] bypasses validation entirely
//! (default: `/health` and `/metrics`). The SSE endpoint `/api/events` is
//! allowed to authenticate via the `?token=...` query parameter in addition
//! to the standard `Authorization: Bearer` header so that browsers using
//! `EventSource` can present a credential.

use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use netpilot_auth::{compute_auth, verify_auth};
use netpilot_config::{AuthAlgorithm, AuthConfig};

/// Axum middleware that enforces bearer-token authentication on every
/// route it is attached to. Routes whose `unauthed_paths` allowlist
/// matches are passed through untouched. The SSE endpoint `/api/events`
/// accepts the token via the `?token=` query parameter as a fallback to
/// the `Authorization: Bearer` header.
pub async fn bearer_auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Snapshot the auth config so we do not hold the read lock across
    // the awaited downstream call.
    let auth = {
        let guard = state.auth.read().await;
        guard.clone()
    };

    let path = req.uri().path().to_string();

    // 1. Allowlist bypass.
    if auth.unauthed_paths.iter().any(|p| p == &path) {
        return Ok(next.run(req).await);
    }

    // 1b. If no bearer secret is configured, the control plane is
    //     considered unauthenticated — every request passes through.
    //     This preserves the pre-C1 behavior for trusted-network
    //     deployments and tests.
    if auth.bearer_secret.is_none() {
        return Ok(next.run(req).await);
    }

    // 2. Token resolution. SSE clients using `EventSource` cannot set
    //    the `Authorization` header, so we also accept `?token=...`.
    let token_opt = extract_token(&req, &path);

    // 3. Validate.
    let token = token_opt.ok_or(StatusCode::UNAUTHORIZED)?;
    validate_bearer(&token, &auth).map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(next.run(req).await)
}

/// Extract the bearer token from either the `Authorization` header (for
/// non-SSE routes) or the `?token=` query parameter (for `/api/events`).
fn extract_token(req: &Request, path: &str) -> Option<String> {
    // Standard `Authorization: Bearer <token>`.
    if let Some(bearer) = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    {
        return Some(bearer.to_string());
    }

    // SSE fallback: `?token=<token>`.
    if path == "/api/events"
        && let Some(query) = req.uri().query()
        && let Some(t) = parse_token_from_query(query)
    {
        return Some(t);
    }

    None
}

/// Validate a `<exp>.<sig_hex>` token against the configured secret.
/// Returns `Err(())` for any structural problem, expiry, or signature
/// mismatch.
fn validate_bearer(token: &str, auth: &AuthConfig) -> Result<(), ()> {
    let secret = auth.bearer_secret.as_deref().ok_or(())?;

    let mut parts = token.splitn(2, '.');
    let exp_str = parts.next().ok_or(())?;
    let sig_hex = parts.next().ok_or(())?;

    let exp: i64 = exp_str.parse().map_err(|_| ())?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    if exp < now {
        return Err(());
    }

    let provided_sig = hex_decode(sig_hex).map_err(|_| ())?;
    let ok = verify_auth(
        &AuthAlgorithm::HmacSha256,
        secret.as_bytes(),
        exp_str.as_bytes(),
        &provided_sig,
    )
    .map_err(|_| ())?;
    if !ok {
        return Err(());
    }

    // Also confirm we can recompute (cheap sanity check on the algorithm
    // path; the verify above is the real gate).
    let _ = compute_auth(
        &AuthAlgorithm::HmacSha256,
        secret.as_bytes(),
        exp_str.as_bytes(),
    );
    Ok(())
}

/// Parse a URL query string and return the value associated with the
/// `token` key, if present.
fn parse_token_from_query(q: &str) -> Option<String> {
    q.split('&').find_map(|p| {
        let mut kv = p.splitn(2, '=');
        if kv.next()? == "token" {
            kv.next().map(|s| s.to_string())
        } else {
            None
        }
    })
}

/// Decode a lowercase hex string into a `Vec<u8>`. Requires an even
/// number of characters.
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

/// Construct a bearer token for the given secret and TTL. The returned
/// string has the form `<exp_unix>.<hex_hmac>` and is what callers should
/// send in the `Authorization: Bearer <token>` header or the
/// `?token=<token>` query parameter.
pub fn generate_bearer_token(secret: &str, ttl_secs: i64) -> Result<String, String> {
    let exp = time::OffsetDateTime::now_utc().unix_timestamp() + ttl_secs;
    let exp_str = exp.to_string();
    let sig = compute_auth(
        &AuthAlgorithm::HmacSha256,
        secret.as_bytes(),
        exp_str.as_bytes(),
    )
    .map_err(|e| format!("compute_auth: {e}"))?;
    Ok(format!("{}.{}", exp_str, hex_encode(&sig)))
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

/// Validate that the server cert and key files referenced by the auth
/// config exist and parse as PEM. This is a fail-fast precheck at
/// startup; the actual TLS handshake is performed by the listener
/// (`axum-server` is the intended integration, tracked as an open item).
pub fn validate_tls_material(
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
) -> Result<(), String> {
    use std::io::BufReader;

    let cert_file = std::fs::File::open(cert_path)
        .map_err(|e| format!("open cert {}: {e}", cert_path.display()))?;
    let certs: Vec<_> = rustls_pemfile::certs(&mut BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("parse cert {}: {e}", cert_path.display()))?;
    if certs.is_empty() {
        return Err(format!("no certificates found in {}", cert_path.display()));
    }

    let key_file = std::fs::File::open(key_path)
        .map_err(|e| format!("open key {}: {e}", key_path.display()))?;
    // Try PKCS#8 first, then fall back to legacy RSA keys.
    let pkcs8_keys: Vec<_> = rustls_pemfile::pkcs8_private_keys(&mut BufReader::new(key_file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("parse pkcs8 key {}: {e}", key_path.display()))?;
    if pkcs8_keys.is_empty() {
        // Re-open and try the legacy RSA format.
        let key_file = std::fs::File::open(key_path)
            .map_err(|e| format!("reopen key {}: {e}", key_path.display()))?;
        let rsa_keys: Vec<_> = rustls_pemfile::rsa_private_keys(&mut BufReader::new(key_file))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("parse rsa key {}: {e}", key_path.display()))?;
        if rsa_keys.is_empty() {
            return Err(format!("no private keys found in {}", key_path.display()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_bearer_token() {
        let secret = "shh";
        let token = generate_bearer_token(secret, 60).expect("token");
        let auth = AuthConfig {
            bearer_secret: Some(secret.into()),
            ..AuthConfig::default()
        };
        assert!(validate_bearer(&token, &auth).is_ok());
    }

    #[test]
    fn expired_token_is_rejected() {
        let secret = "shh";
        let token = generate_bearer_token(secret, -10).expect("token");
        let auth = AuthConfig {
            bearer_secret: Some(secret.into()),
            ..AuthConfig::default()
        };
        assert!(validate_bearer(&token, &auth).is_err());
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let token = generate_bearer_token("a", 60).expect("token");
        let auth = AuthConfig {
            bearer_secret: Some("b".into()),
            ..AuthConfig::default()
        };
        assert!(validate_bearer(&token, &auth).is_err());
    }

    #[test]
    fn missing_secret_is_rejected_at_validate_bearer() {
        let auth = AuthConfig::default();
        // The `validate_bearer` helper itself rejects tokens when the
        // secret is missing; the middleware short-circuits before
        // calling it. Both behaviors are intentional defense in depth.
        assert!(validate_bearer("123.aaaa", &auth).is_err());
    }

    #[test]
    fn hex_round_trip() {
        let s = hex_encode(&[0x00, 0xff, 0xab, 0xcd]);
        assert_eq!(s, "00ffabcd");
        assert_eq!(hex_decode(&s).unwrap(), vec![0x00, 0xff, 0xab, 0xcd]);
        assert!(hex_decode("abc").is_err()); // odd length
    }

    #[test]
    fn parse_token_query_basic() {
        assert_eq!(
            parse_token_from_query("token=abc123").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            parse_token_from_query("foo=bar&token=zz&baz=qux").as_deref(),
            Some("zz")
        );
        assert!(parse_token_from_query("foo=bar").is_none());
    }
}
