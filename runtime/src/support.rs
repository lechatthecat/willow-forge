//! Small helpers shared across generated apps.

use sha2::{Digest, Sha256};
use uuid::Uuid;

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
    let digest = Sha256::digest(email.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
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
}
