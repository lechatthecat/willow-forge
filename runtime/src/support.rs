//! Small helpers shared across generated apps.

use uuid::Uuid;

/// Generate a random, URL-safe token (128-bit, 32 lowercase hex chars).
///
/// Suitable for one-time links such as password-reset and email-verification.
/// Store a hash of the token (e.g. via [`crate::Hash::make`]) rather than the
/// raw value, and compare with [`crate::Hash::check`] on redemption.
pub fn random_token() -> String {
    Uuid::new_v4().simple().to_string()
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
}
