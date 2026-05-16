use std::sync::Arc;

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use redis::AsyncCommands;
use redis::cluster::ClusterClient;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_errors::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,
    pub jti: String,
    pub exp: usize,
}

pub struct Jwt;

impl Jwt {
    pub fn encode(user_id: i64) -> Result<String, AppError> {
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
        let expiry: u64 = std::env::var("JWT_EXPIRY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);

        let exp = (chrono::Utc::now().timestamp() as u64 + expiry) as usize;
        let claims = Claims {
            sub: user_id,
            jti: Uuid::new_v4().to_string(),
            exp,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|e| {
            tracing::error!("JWT encode error: {}", e);
            AppError::Internal
        })
    }

    pub fn decode(token: &str) -> Result<Claims, AppError> {
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());

        decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| {
            tracing::warn!("JWT decode error: {}", e);
            AppError::Unauthorized
        })
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

    pub async fn is_blacklisted(
        jti: &str,
        redis: &Arc<ClusterClient>,
    ) -> Result<bool, AppError> {
        let key = format!("jwt:blacklist:{}", jti);
        let mut conn = redis.get_async_connection().await?;
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        unsafe { std::env::set_var("JWT_SECRET", "test-secret") };
        let token = Jwt::encode(42).unwrap();
        let claims = Jwt::decode(&token).unwrap();
        assert_eq!(claims.sub, 42);
        assert!(!claims.jti.is_empty());
    }

    #[test]
    fn invalid_token_returns_unauthorized() {
        unsafe { std::env::set_var("JWT_SECRET", "test-secret") };
        let result = Jwt::decode("not.a.token");
        assert!(result.is_err());
    }
}
