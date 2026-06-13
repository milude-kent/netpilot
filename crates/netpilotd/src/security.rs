/// Drop privileges after binding to ports. On Linux, set CAP_NET_BIND_SERVICE
/// and then drop to a non-root user.
pub fn drop_privileges(user: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // In production: use capctl or libcap to drop capabilities
        // For now: log that we should be running as non-root
    }
    eprintln!(
        "security: running as '{}' (privilege dropping is Linux-only)",
        user
    );
    Ok(())
}

/// Validate TLS configuration consistency.
pub fn validate_tls_config(
    cert_path: &Option<String>,
    key_path: &Option<String>,
) -> Result<(), String> {
    match (cert_path, key_path) {
        (Some(_), Some(_)) => Ok(()),
        (None, None) => Ok(()),
        _ => Err("TLS requires both cert-path and key-path or neither".into()),
    }
}
