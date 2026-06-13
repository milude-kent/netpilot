//! Integration tests for `netpilot-auth`.
//!
//! The vectors here are drawn from the public standards so we can catch
//! regressions in the underlying HMAC / Blake2 wiring even when the rest of
//! the workspace is not being exercised.
//!
//! Sources:
//!   * RFC 2202 — HMAC-MD5 / HMAC-SHA-1 test cases
//!   * RFC 4231 — HMAC-SHA-2 test cases
//!   * Blake2 round-trip — no public KAT for the MAC mode with a 64-byte
//!     key is sourced here; we use a round-trip test to confirm that
//!     compute/verify agree on a key+data pair.

use netpilot_auth::{AuthError, compute_auth, verify_auth};
use netpilot_config::AuthAlgorithm;

fn hex_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let pair = std::str::from_utf8(&bytes[i..i + 2]).expect("valid hex pair");
        out.push(u8::from_str_radix(pair, 16).expect("valid hex digit"));
        i += 2;
    }
    out
}

// ---------------------------------------------------------------------------
// HMAC-SHA-256 — RFC 4231 §4.2, test case 1
// ---------------------------------------------------------------------------

#[test]
fn rfc4231_hmac_sha256_case1() {
    let key = vec![0x0b; 20];
    let data = b"Hi There";
    let expected = hex_decode("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");

    let mac = compute_auth(&AuthAlgorithm::HmacSha256, &key, data).expect("compute ok");
    assert_eq!(
        mac, expected,
        "HMAC-SHA-256 mismatch with RFC 4231 §4.2 case 1"
    );
    assert!(verify_auth(&AuthAlgorithm::HmacSha256, &key, data, &expected).unwrap());
}

// ---------------------------------------------------------------------------
// HMAC-SHA-512 — RFC 4231 §4.2, test case 1
// ---------------------------------------------------------------------------

#[test]
fn rfc4231_hmac_sha512_case1() {
    let key = vec![0x0b; 20];
    let data = b"Hi There";
    let expected = hex_decode(
        "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854",
    );

    let mac = compute_auth(&AuthAlgorithm::HmacSha512, &key, data).expect("compute ok");
    assert_eq!(
        mac, expected,
        "HMAC-SHA-512 mismatch with RFC 4231 §4.2 case 1"
    );
    assert!(verify_auth(&AuthAlgorithm::HmacSha512, &key, data, &expected).unwrap());
}

// ---------------------------------------------------------------------------
// HMAC-MD5 — RFC 2202 §2, test case 1
// ---------------------------------------------------------------------------

#[test]
fn rfc2202_hmac_md5_case1() {
    let key = vec![0x0b; 16];
    let data = b"Hi There";
    let expected = hex_decode("9294727a3638bb1c13f48ef8158bfc9d");

    let mac = compute_auth(&AuthAlgorithm::KeyedMd5, &key, data).expect("compute ok");
    assert_eq!(mac, expected, "HMAC-MD5 mismatch with RFC 2202 §2 case 1");
    assert!(verify_auth(&AuthAlgorithm::KeyedMd5, &key, data, &expected).unwrap());
}

// ---------------------------------------------------------------------------
// HMAC-SHA-1 — RFC 2202 §3, test case 1
// ---------------------------------------------------------------------------

#[test]
fn rfc2202_hmac_sha1_case1() {
    let key = vec![0x0b; 20];
    let data = b"Hi There";
    let expected = hex_decode("b617318655057264e28bc0b6fb378c8ef146be00");

    let mac = compute_auth(&AuthAlgorithm::KeyedSha1, &key, data).expect("compute ok");
    assert_eq!(mac, expected, "HMAC-SHA-1 mismatch with RFC 2202 §3 case 1");
    assert!(verify_auth(&AuthAlgorithm::KeyedSha1, &key, data, &expected).unwrap());
}

// ---------------------------------------------------------------------------
// HMAC-SHA-384 — RFC 4231 §4.2, test case 1 (added for breadth)
// ---------------------------------------------------------------------------

#[test]
fn rfc4231_hmac_sha384_case1() {
    let key = vec![0x0b; 20];
    let data = b"Hi There";
    let expected = hex_decode(
        "afd03944d84895626b0825f4ab46907f15f9dadbe4101ec682aa034c7cebc59cfaea9ea9076ede7f4af152e8b2fa9cb6",
    );

    let mac = compute_auth(&AuthAlgorithm::HmacSha384, &key, data).expect("compute ok");
    assert_eq!(
        mac, expected,
        "HMAC-SHA-384 mismatch with RFC 4231 §4.2 case 1"
    );
}

// ---------------------------------------------------------------------------
// Blake2b-512 — round-trip (no public KAT sourced for this code path).
// ---------------------------------------------------------------------------

#[test]
fn blake2b_512_round_trip() {
    let key = b"a-blake2b-key-with-enough-bytes-to-be-valid";
    let data = b"OSPFv3 authentication payload";
    let mac = compute_auth(&AuthAlgorithm::Blake2b512, key, data).expect("compute ok");
    assert_eq!(mac.len(), 64, "Blake2b-512 should emit 64 bytes");
    assert!(verify_auth(&AuthAlgorithm::Blake2b512, key, data, &mac).unwrap());
}

// ---------------------------------------------------------------------------
// Blake2b-256 — round-trip, length check
// ---------------------------------------------------------------------------

#[test]
fn blake2b_256_round_trip() {
    let key = b"another-blake2b-key";
    let data = b"data";
    let mac = compute_auth(&AuthAlgorithm::Blake2b256, key, data).expect("compute ok");
    assert_eq!(mac.len(), 32, "Blake2b-256 should emit 32 bytes");
    assert!(verify_auth(&AuthAlgorithm::Blake2b256, key, data, &mac).unwrap());
}

// ---------------------------------------------------------------------------
// Blake2s-128 / Blake2s-256 — round-trip, length check
// ---------------------------------------------------------------------------

#[test]
fn blake2s_128_round_trip() {
    let key = b"blake2s-key";
    let data = b"data";
    let mac = compute_auth(&AuthAlgorithm::Blake2s128, key, data).expect("compute ok");
    assert_eq!(mac.len(), 16, "Blake2s-128 should emit 16 bytes");
    assert!(verify_auth(&AuthAlgorithm::Blake2s128, key, data, &mac).unwrap());
}

#[test]
fn blake2s_256_round_trip() {
    let key = b"blake2s-key";
    let data = b"data";
    let mac = compute_auth(&AuthAlgorithm::Blake2s256, key, data).expect("compute ok");
    assert_eq!(mac.len(), 32, "Blake2s-256 should emit 32 bytes");
    assert!(verify_auth(&AuthAlgorithm::Blake2s256, key, data, &mac).unwrap());
}

// ---------------------------------------------------------------------------
// Constant-time round-trip plus single-bit-flip detection.
// ---------------------------------------------------------------------------

#[test]
fn constant_time_roundtrip_then_flip() {
    let key = b"session-key";
    let data = b"protocol payload";
    let algo = AuthAlgorithm::HmacSha256;

    let mac = compute_auth(&algo, key, data).expect("compute ok");
    assert!(
        verify_auth(&algo, key, data, &mac).unwrap(),
        "round-trip must succeed"
    );

    // Flip the low bit of the first byte and confirm verify returns false.
    let mut tampered = mac.clone();
    tampered[0] ^= 0x01;
    assert!(
        !verify_auth(&algo, key, data, &tampered).unwrap(),
        "single-bit-flip must fail verification"
    );
}

// ---------------------------------------------------------------------------
// Wrong-key fail path.
// ---------------------------------------------------------------------------

#[test]
fn wrong_key_fails_verification() {
    let data = b"some packet body";
    let algo = AuthAlgorithm::HmacSha256;
    let mac = compute_auth(&algo, b"correct-key", data).expect("compute ok");

    assert!(
        !verify_auth(&algo, b"incorrect-key", data, &mac).unwrap(),
        "verifying with the wrong key must return false"
    );
}

// ---------------------------------------------------------------------------
// Wrong-expected-length short-circuit (still returns Ok(false), not Err).
// ---------------------------------------------------------------------------

#[test]
fn wrong_length_expected_returns_false() {
    let algo = AuthAlgorithm::HmacSha256;
    let result = verify_auth(&algo, b"k", b"d", &[0u8; 4]).expect("verify must not error");
    assert!(
        !result,
        "shorter-than-MAC expected buffer must fail verification"
    );
}

// ---------------------------------------------------------------------------
// KeyedMd5 / KeyedSha1 round-trip — uses the proper HMAC construction
// (RFC 2104) rather than the legacy H(key || data) pattern.
// ---------------------------------------------------------------------------

#[test]
fn keyed_md5_round_trip_succeeds_with_correct_expected() {
    let key = vec![0x0b; 16];
    let data = b"Hi There";
    let algo = AuthAlgorithm::KeyedMd5;

    let mac = compute_auth(&algo, &key, data).expect("compute ok");
    assert_eq!(mac.len(), 16);
    assert!(verify_auth(&algo, &key, data, &mac).unwrap());
}

#[test]
fn keyed_sha1_round_trip_succeeds_with_correct_expected() {
    let key = vec![0x0b; 20];
    let data = b"Hi There";
    let algo = AuthAlgorithm::KeyedSha1;

    let mac = compute_auth(&algo, &key, data).expect("compute ok");
    assert_eq!(mac.len(), 20);
    assert!(verify_auth(&algo, &key, data, &mac).unwrap());
}

// ---------------------------------------------------------------------------
// Error-type coverage.
// ---------------------------------------------------------------------------

#[test]
fn invalid_key_is_returned_for_oversized_blake2_key() {
    // Blake2 key length is bounded by the underlying block size (128 bytes
    // for Blake2b, 64 bytes for Blake2s). Pushing past that should produce
    // AuthError::InvalidKey.
    let oversize = vec![0u8; 256];
    let err = compute_auth(&AuthAlgorithm::Blake2b512, &oversize, b"data")
        .expect_err("oversized key must be rejected");
    match err {
        AuthError::InvalidKey(msg) => assert!(!msg.is_empty()),
        other => panic!("expected InvalidKey, got {other:?}"),
    }
}
