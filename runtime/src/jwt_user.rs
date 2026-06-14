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

        let claims = Jwt::decode_with_config(token, &state.config.jwt).map_err(|_| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode, header}, response::IntoResponse, routing::get, Router};
    use std::sync::Arc;
    use tower::ServiceExt;
    use crate::app_state::{AppState, Config, Services};
    use crate::jwt::Jwt;

    fn dummy_state() -> Arc<AppState> {
        let db = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://fake:fake@127.0.0.1:5432/fake")
            .unwrap();
        let redis = Arc::new(
            redis::cluster::ClusterClient::new(vec!["redis://127.0.0.1:9999/"])
                .unwrap(),
        );
        let config = Config {
            app_name: "test".to_string(),
            app_env: "test".to_string(),
            app_debug: false,
            jwt: crate::app_state::JwtConfig {
                secret: "test-secret".to_string(),
                expiry: 3600,
            },
            ..Config::default()
        };

        Arc::new(AppState {
            config,
            services: Services {
                db,
                redis,
                mailer: crate::mailer::Mailer::from_config(&crate::mailer::MailConfig::default())
                    .unwrap(),
            },
            views: minijinja::Environment::new(),
        })
    }

    async fn jwt_handler(_auth: JwtUser) -> impl IntoResponse {
        StatusCode::OK
    }

    fn jwt_app() -> Router {
        Router::new()
            .route("/api/me", get(jwt_handler))
            .with_state(dummy_state())
    }

    #[tokio::test]
    async fn ju_01_no_auth_header_returns_401() {
        let req = Request::builder().uri("/api/me").body(Body::empty()).unwrap();
        assert_eq!(jwt_app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ju_02_non_bearer_format_returns_401() {
        let req = Request::builder()
            .uri("/api/me")
            .header(header::AUTHORIZATION, "Token some-token")
            .body(Body::empty())
            .unwrap();
        assert_eq!(jwt_app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ju_03_invalid_jwt_returns_401() {
        let req = Request::builder()
            .uri("/api/me")
            .header(header::AUTHORIZATION, "Bearer not.a.valid.jwt")
            .body(Body::empty())
            .unwrap();
        assert_eq!(jwt_app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ju_04_valid_jwt_with_unreachable_redis_returns_200() {
        // is_blacklisted() returns Err when Redis is unreachable.
        // unwrap_or(false) treats the failure as "not blacklisted" — request proceeds.
        let token = Jwt::encode_with_config(1, &dummy_state().config.jwt).unwrap();
        let req = Request::builder()
            .uri("/api/me")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        assert_eq!(jwt_app().oneshot(req).await.unwrap().status(), StatusCode::OK);
    }
}
