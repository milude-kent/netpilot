//! Protocol authentication primitives — HMAC, Keyed-MD5/SHA-1 (proper HMAC),
//! and Blake2 MAC variants. All comparisons run in constant time via
//! [`subtle::ConstantTimeEq`]; computed MAC buffers are zeroized on drop.

use blake2::digest::{FixedOutput, KeyInit, Update};
use blake2::{Blake2bMac, Blake2sMac};
use digest::consts::{U16, U32, U64};
use hmac::{Hmac, Mac};
pub use netpilot_config::AuthAlgorithm;
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

type HmacMd5 = Hmac<md5::Md5>;
type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type HmacSha384 = Hmac<Sha384>;
type HmacSha512 = Hmac<Sha512>;

// Blake2 MAC type aliases keyed to the variants declared in the schema.
type Blake2sMac128 = Blake2sMac<U16>;
type Blake2sMac256 = Blake2sMac<U32>;
type Blake2bMac256 = Blake2bMac<U32>;
type Blake2bMac512 = Blake2bMac<U64>;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("authentication failed")]
    AuthFailed,
    #[error("unsupported algorithm: {0:?}")]
    UnsupportedAlgorithm(AuthAlgorithm),
    #[error("invalid key: {0}")]
    InvalidKey(String),
}

/// Compute a keyed MAC for protocol authentication.
///
/// The returned `Vec<u8>` carries sensitive material; callers that retain
/// the buffer beyond a single comparison should wrap it in
/// [`zeroize::Zeroizing`] so the bytes are wiped on drop.
pub fn compute_auth(
    algorithm: &AuthAlgorithm,
    key: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, AuthError> {
    match algorithm {
        AuthAlgorithm::HmacSha1 => hmac_finalize::<HmacSha1>(key, data),
        AuthAlgorithm::HmacSha256 => hmac_finalize::<HmacSha256>(key, data),
        AuthAlgorithm::HmacSha384 => hmac_finalize::<HmacSha384>(key, data),
        AuthAlgorithm::HmacSha512 => hmac_finalize::<HmacSha512>(key, data),
        // BIRD/Quagga keyed-{md5,sha1} are HMAC constructions — NOT
        // raw H(key||data). The old behaviour was vulnerable to
        // length-extension; we now use proper HMAC.
        AuthAlgorithm::KeyedMd5 => hmac_finalize::<HmacMd5>(key, data),
        AuthAlgorithm::KeyedSha1 => hmac_finalize::<HmacSha1>(key, data),
        AuthAlgorithm::Blake2s128 => blake2_finalize::<Blake2sMac128>(key, data),
        AuthAlgorithm::Blake2s256 => blake2_finalize::<Blake2sMac256>(key, data),
        AuthAlgorithm::Blake2b256 => blake2_finalize::<Blake2bMac256>(key, data),
        AuthAlgorithm::Blake2b512 => blake2_finalize::<Blake2bMac512>(key, data),
    }
}

fn hmac_finalize<M: Mac + KeyInit>(key: &[u8], data: &[u8]) -> Result<Vec<u8>, AuthError> {
    let mut mac =
        <M as Mac>::new_from_slice(key).map_err(|e| AuthError::InvalidKey(e.to_string()))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn blake2_finalize<M: KeyInit + Update + FixedOutput>(
    key: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, AuthError> {
    let mut mac =
        <M as KeyInit>::new_from_slice(key).map_err(|e| AuthError::InvalidKey(e.to_string()))?;
    mac.update(data);
    Ok(mac.finalize_fixed().to_vec())
}

/// Verify an authentication digest in constant time.
///
/// Returns `Ok(true)` if `expected` equals the recomputed MAC, `Ok(false)`
/// otherwise. The length-mismatch short-circuit is safe because the MAC
/// length is a public function of the algorithm.
pub fn verify_auth(
    algorithm: &AuthAlgorithm,
    key: &[u8],
    data: &[u8],
    expected: &[u8],
) -> Result<bool, AuthError> {
    let computed = Zeroizing::new(compute_auth(algorithm, key, data)?);
    if computed.len() != expected.len() {
        tracing::warn!(
            algorithm = ?algorithm,
            expected_len = expected.len(),
            computed_len = computed.len(),
            "auth verify failed: length mismatch"
        );
        return Ok(false);
    }
    let eq: bool = computed.as_slice().ct_eq(expected).into();
    if !eq {
        tracing::warn!(algorithm = ?algorithm, "auth verify failed: MAC mismatch");
    }
    Ok(eq)
}

/// Look up the active password for a protocol from its password list.
///
/// By convention an inline `password = "..."` config field is treated as
/// HMAC-SHA-256 when no algorithm is specified; tuned per-password entries
/// in the `passwords = [...]` list override this default.
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

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

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
    fn flipped_byte_fails_verification() {
        let key = b"k";
        let data = b"payload";
        let mut hash = compute_auth(&AuthAlgorithm::HmacSha256, key, data).unwrap();
        hash[0] ^= 0x01;
        assert!(!verify_auth(&AuthAlgorithm::HmacSha256, key, data, &hash).unwrap());
    }

    #[test]
    fn resolve_simple_password() {
        let password = Some("mypass".into());
        let result = resolve_password(&[], &password);
        assert_eq!(result.unwrap().1, "mypass");
    }

    // RFC 4231 §4.2 test case 1
    #[test]
    fn hmac_sha256_rfc4231_case1() {
        let key = vec![0x0b; 20];
        let data = b"Hi There";
        let expected =
            hex_decode("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
        let mac = compute_auth(&AuthAlgorithm::HmacSha256, &key, data).unwrap();
        assert_eq!(mac, expected);
    }

    // RFC 4231 §4.2 test case 1
    #[test]
    fn hmac_sha512_rfc4231_case1() {
        let key = vec![0x0b; 20];
        let data = b"Hi There";
        let expected = hex_decode(
            "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854",
        );
        let mac = compute_auth(&AuthAlgorithm::HmacSha512, &key, data).unwrap();
        assert_eq!(mac, expected);
    }

    // RFC 2202 test case 1 — HMAC-MD5 = HMAC(key=16x0x0b, "Hi There")
    #[test]
    fn hmac_md5_rfc2202_case1() {
        let key = vec![0x0b; 16];
        let data = b"Hi There";
        let expected = hex_decode("9294727a3638bb1c13f48ef8158bfc9d");
        let mac = compute_auth(&AuthAlgorithm::KeyedMd5, &key, data).unwrap();
        assert_eq!(mac, expected, "KeyedMd5 must now be real HMAC-MD5");
    }

    // RFC 2202 test case 1 — HMAC-SHA-1 = HMAC(key=20x0x0b, "Hi There")
    #[test]
    fn hmac_sha1_rfc2202_case1() {
        let key = vec![0x0b; 20];
        let data = b"Hi There";
        let expected = hex_decode("b617318655057264e28bc0b6fb378c8ef146be00");
        let mac = compute_auth(&AuthAlgorithm::KeyedSha1, &key, data).unwrap();
        assert_eq!(mac, expected, "KeyedSha1 must now be real HMAC-SHA-1");
    }

    #[test]
    fn keyed_md5_round_trip() {
        let key = b"some-key";
        let data = b"payload";
        let mac = compute_auth(&AuthAlgorithm::KeyedMd5, key, data).unwrap();
        assert!(verify_auth(&AuthAlgorithm::KeyedMd5, key, data, &mac).unwrap());
    }

    #[test]
    fn keyed_sha1_round_trip() {
        let key = b"some-key";
        let data = b"payload";
        let mac = compute_auth(&AuthAlgorithm::KeyedSha1, key, data).unwrap();
        assert!(verify_auth(&AuthAlgorithm::KeyedSha1, key, data, &mac).unwrap());
    }

    #[test]
    fn blake2s_128_round_trip() {
        let key = b"k";
        let data = b"payload";
        let mac = compute_auth(&AuthAlgorithm::Blake2s128, key, data).unwrap();
        assert_eq!(mac.len(), 16);
        assert!(verify_auth(&AuthAlgorithm::Blake2s128, key, data, &mac).unwrap());
    }

    #[test]
    fn blake2b_512_round_trip() {
        let key = b"k";
        let data = b"payload";
        let mac = compute_auth(&AuthAlgorithm::Blake2b512, key, data).unwrap();
        assert_eq!(mac.len(), 64);
        assert!(verify_auth(&AuthAlgorithm::Blake2b512, key, data, &mac).unwrap());
    }

    #[test]
    fn oversized_blake2_key_returns_invalid_key() {
        // Blake2b key max length is 64 bytes per spec.
        let oversized = vec![0xAA; 256];
        let data = b"x";
        let err = compute_auth(&AuthAlgorithm::Blake2b512, &oversized, data).unwrap_err();
        assert!(matches!(err, AuthError::InvalidKey(_)));
    }
}
