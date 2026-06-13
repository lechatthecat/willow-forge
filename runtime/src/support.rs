//! Small helpers shared across generated apps.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Lowercase hex encoding of a byte slice.
fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Decode a lowercase/uppercase hex string into bytes. Returns `None` if the
/// input is not valid hex (odd length or non-hex digit).
fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Generate a random, URL-safe token (128-bit, 32 lowercase hex chars).
///
/// Suitable for one-time links such as password-reset and email-verification.
/// Store a hash of the token (e.g. via [`crate::Hash::make`]) rather than the
/// raw value, and compare with [`crate::Hash::check`] on redemption.
pub fn random_token() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Deterministic SHA-256 hex digest of an email address.
///
/// Used to build stateless email-verification links of the form
/// `/email/verify/{id}/{hash}`: the hash binds the link to the user's current
/// email, so it stops working if the address changes. Compare the path hash
/// against `email_verification_hash(user.email)` on the verify route.
pub fn email_verification_hash(email: &str) -> String {
    to_hex(&Sha256::digest(email.as_bytes()))
}

/// HMAC-SHA256 hex signature of `payload` keyed by `key`.
///
/// Use to sign tamper-proof, expiring links (e.g. the `signature` query param
/// on a verification URL). Verify with [`verify_signature`].
pub fn sign(payload: &str, key: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(payload.as_bytes());
    to_hex(&mac.finalize().into_bytes())
}

/// Constant-time verification of a hex [`sign`] signature. Returns `false` for
/// any tampering, wrong key, or malformed (non-hex) signature.
pub fn verify_signature(payload: &str, signature: &str, key: &str) -> bool {
    let Some(sig_bytes) = from_hex(signature) else {
        return false;
    };
    let mut mac =
        HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(payload.as_bytes());
    mac.verify_slice(&sig_bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_32_hex_chars() {
        let t = random_token();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn tokens_are_unique() {
        let a = random_token();
        let b = random_token();
        assert_ne!(a, b);
    }

    #[test]
    fn verification_hash_is_64_hex_chars() {
        let h = email_verification_hash("alice@example.com");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn verification_hash_is_deterministic() {
        assert_eq!(
            email_verification_hash("alice@example.com"),
            email_verification_hash("alice@example.com"),
        );
    }

    #[test]
    fn verification_hash_differs_by_email() {
        assert_ne!(
            email_verification_hash("alice@example.com"),
            email_verification_hash("bob@example.com"),
        );
    }

    #[test]
    fn verification_hash_known_vector() {
        // SHA-256 of "" is the well-known empty-string digest.
        assert_eq!(
            email_verification_hash(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }

    #[test]
    fn sign_roundtrips() {
        let sig = sign("1.abc.123", "secret");
        assert!(verify_signature("1.abc.123", &sig, "secret"));
    }

    #[test]
    fn sign_is_64_hex_chars() {
        let sig = sign("payload", "key");
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let sig = sign("payload", "key-a");
        assert!(!verify_signature("payload", &sig, "key-b"));
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let sig = sign("1.abc.123", "secret");
        assert!(!verify_signature("1.abc.124", &sig, "secret"));
    }

    #[test]
    fn verify_rejects_malformed_signature() {
        assert!(!verify_signature("payload", "not-hex!!", "key"));
        assert!(!verify_signature("payload", "abc", "key")); // odd length
    }

    #[test]
    fn sign_known_vector() {
        // RFC 4231-style check: HMAC-SHA256 is deterministic for fixed key+msg.
        assert_eq!(
            sign("The quick brown fox jumps over the lazy dog", "key"),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
        );
    }
}
