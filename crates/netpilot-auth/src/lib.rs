use digest::Digest;
use hmac::{Hmac, Mac};
use netpilot_config::AuthAlgorithm;
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type HmacSha384 = Hmac<Sha384>;
type HmacSha512 = Hmac<Sha512>;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("authentication failed")]
    AuthFailed,
    #[error("unsupported algorithm: {0:?}")]
    UnsupportedAlgorithm(AuthAlgorithm),
}

/// Compute a keyed hash for protocol authentication.
pub fn compute_auth(
    algorithm: &AuthAlgorithm,
    key: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, AuthError> {
    match algorithm {
        AuthAlgorithm::HmacSha1 => {
            let mut mac = HmacSha1::new_from_slice(key).map_err(|_| AuthError::AuthFailed)?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        AuthAlgorithm::HmacSha256 => {
            let mut mac = HmacSha256::new_from_slice(key).map_err(|_| AuthError::AuthFailed)?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        AuthAlgorithm::HmacSha384 => {
            let mut mac = HmacSha384::new_from_slice(key).map_err(|_| AuthError::AuthFailed)?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        AuthAlgorithm::HmacSha512 => {
            let mut mac = HmacSha512::new_from_slice(key).map_err(|_| AuthError::AuthFailed)?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        AuthAlgorithm::KeyedMd5 => {
            // MD5 keyed hash
            let mut mac = md5::Md5::new();
            mac.update(key);
            mac.update(data);
            Ok(mac.finalize().to_vec())
        }
        AuthAlgorithm::KeyedSha1 => {
            let mut mac = sha1::Sha1::new();
            mac.update(key);
            mac.update(data);
            Ok(mac.finalize().to_vec())
        }
        _ => Err(AuthError::UnsupportedAlgorithm(algorithm.clone())),
    }
}

/// Verify an authentication digest.
pub fn verify_auth(
    algorithm: &AuthAlgorithm,
    key: &[u8],
    data: &[u8],
    expected: &[u8],
) -> Result<bool, AuthError> {
    let computed = compute_auth(algorithm, key, data)?;
    Ok(computed == expected)
}

/// Look up the active password for a protocol from its password list.
pub fn resolve_password<'a>(
    passwords: &'a [netpilot_config::AuthPassword],
    password: &'a Option<String>,
) -> Option<(AuthAlgorithm, &'a str)> {
    if let Some(p) = password {
        return Some((AuthAlgorithm::HmacSha256, p.as_str()));
    }
    passwords.first().map(|p| {
        (
            p.algorithm.clone().unwrap_or(AuthAlgorithm::HmacSha256),
            p.password.as_str(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_auth_round_trips() {
        let key = b"secret";
        let data = b"hello ospf";
        let hash = compute_auth(&AuthAlgorithm::HmacSha256, key, data).unwrap();
        assert!(verify_auth(&AuthAlgorithm::HmacSha256, key, data, &hash).unwrap());
    }

    #[test]
    fn wrong_key_fails_verification() {
        let data = b"hello";
        let hash = compute_auth(&AuthAlgorithm::HmacSha256, b"right", data).unwrap();
        assert!(!verify_auth(&AuthAlgorithm::HmacSha256, b"wrong", data, &hash).unwrap());
    }

    #[test]
    fn resolve_simple_password() {
        let password = Some("mypass".into());
        let result = resolve_password(&[], &password);
        assert_eq!(result.unwrap().1, "mypass");
    }
}
