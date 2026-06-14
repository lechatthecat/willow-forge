use std::sync::Arc;

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use redis::AsyncCommands;
use redis::cluster::ClusterClient;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{app_errors::AppError, app_state::JwtConfig};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,
    pub jti: String,
    pub exp: usize,
}

pub struct Jwt;

impl Jwt {
    pub fn encode(user_id: i64) -> Result<String, AppError> {
        let config = JwtConfig {
            secret: std::env::var("JWT_SECRET").map_err(|_| {
                tracing::error!("JWT_SECRET is required before issuing JWTs");
                AppError::Internal
            })?,
            expiry: std::env::var("JWT_EXPIRY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
        };

        Self::encode_with_config(user_id, &config)
    }

    pub fn encode_with_config(user_id: i64, config: &JwtConfig) -> Result<String, AppError> {
        Self::validate_secret(config)?;

        let exp = (chrono::Utc::now().timestamp() as u64 + config.expiry) as usize;
        let claims = Claims {
            sub: user_id,
            jti: Uuid::new_v4().to_string(),
            exp,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(config.secret.as_bytes()),
        )
        .map_err(|e| {
            tracing::error!("JWT encode error: {}", e);
            AppError::Internal
        })
    }

    pub fn decode(token: &str) -> Result<Claims, AppError> {
        let config = JwtConfig {
            secret: std::env::var("JWT_SECRET").map_err(|_| {
                tracing::error!("JWT_SECRET is required before verifying JWTs");
                AppError::Unauthorized
            })?,
            expiry: 3600,
        };

        Self::decode_with_config(token, &config)
    }

    pub fn decode_with_config(token: &str, config: &JwtConfig) -> Result<Claims, AppError> {
        Self::validate_secret(config)?;

        decode::<Claims>(
            token,
            &DecodingKey::from_secret(config.secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| {
            tracing::warn!("JWT decode error: {}", e);
            AppError::Unauthorized
        })
    }

    pub fn validate_secret(config: &JwtConfig) -> Result<(), AppError> {
        let secret = config.secret.trim();
        let is_placeholder = matches!(secret, "secret" | "change-me-in-production");

        if secret.len() < 32 || is_placeholder {
            tracing::error!(
                "JWT secret is not configured securely; set JWT_SECRET to at least 32 random characters"
            );
            return Err(AppError::Internal);
        }

        Ok(())
    }

    pub async fn blacklist(
        jti: &str,
        remaining_secs: u64,
        redis: &Arc<ClusterClient>,
    ) -> Result<(), AppError> {
        if remaining_secs == 0 {
            return Ok(());
        }
        let key = format!("jwt:blacklist:{}", jti);
        let mut conn = redis.get_async_connection().await?;
        let _: () = conn.set_ex(key, 1u8, remaining_secs).await?;
        Ok(())
    }

    pub async fn is_blacklisted(jti: &str, redis: &Arc<ClusterClient>) -> Result<bool, AppError> {
        let key = format!("jwt:blacklist:{}", jti);
        let mut conn = redis.get_async_connection().await?;
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-for-jwt-tests-1234567890";

    #[test]
    fn encode_decode_roundtrip() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET) };
        let token = Jwt::encode(42).unwrap();
        let claims = Jwt::decode(&token).unwrap();
        assert_eq!(claims.sub, 42);
        assert!(!claims.jti.is_empty());
    }

    #[test]
    fn invalid_token_returns_unauthorized() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET) };
        let result = Jwt::decode("not.a.token");
        assert!(result.is_err());
    }

    #[test]
    fn weak_secret_is_rejected() {
        let config = JwtConfig {
            secret: "change-me-in-production".to_string(),
            expiry: 3600,
        };

        assert!(matches!(Jwt::encode_with_config(1, &config), Err(AppError::Internal)));
    }
}
