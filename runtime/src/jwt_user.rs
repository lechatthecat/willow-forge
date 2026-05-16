use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::app_state::AppState;
use crate::jwt::Jwt;

pub struct JwtUser {
    pub id: i64,
    pub jti: String,
}

impl FromRequestParts<Arc<AppState>> for JwtUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "message": "Unauthenticated." })),
                )
                    .into_response()
            })?;

        let claims = Jwt::decode(token).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "message": "Invalid or expired token." })),
            )
                .into_response()
        })?;

        let blacklisted = Jwt::is_blacklisted(&claims.jti, &state.services.redis)
            .await
            .unwrap_or(false);

        if blacklisted {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "message": "Token has been revoked." })),
            )
                .into_response());
        }

        Ok(JwtUser { id: claims.sub, jti: claims.jti })
    }
}
