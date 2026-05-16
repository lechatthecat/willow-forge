use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::app_errors::AppError;

pub struct Hash;

impl Hash {
    /// Hash a password with Argon2id. Returns the PHC string (includes algorithm, salt, hash).
    pub fn make(password: &str) -> Result<String, AppError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                tracing::error!("Password hashing failed: {}", e);
                AppError::Internal
            })?;
        Ok(hash.to_string())
    }

    /// Verify a plain-text password against a stored PHC hash. Returns false on any mismatch or error.
    pub fn check(password: &str, hash: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_and_check_roundtrip() {
        let hash = Hash::make("secret123").unwrap();
        assert!(Hash::check("secret123", &hash));
    }

    #[test]
    fn wrong_password_returns_false() {
        let hash = Hash::make("correct").unwrap();
        assert!(!Hash::check("wrong", &hash));
    }

    #[test]
    fn malformed_hash_returns_false() {
        assert!(!Hash::check("password", "not-a-valid-phc-string"));
    }

    #[test]
    fn empty_password_hashes_and_verifies() {
        let hash = Hash::make("").unwrap();
        assert!(Hash::check("", &hash));
        assert!(!Hash::check("notempty", &hash));
    }
}
