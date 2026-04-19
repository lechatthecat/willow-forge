use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::validated_json::ValidationError;
use crate::view::ViewError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found")]
    NotFound,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Validation failed")]
    Validation(#[from] ValidationError),

    #[error("View render failed")]
    View(#[from] ViewError),

    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Redis error")]
    Redis(#[from] redis::RedisError),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal server error")]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": "Not found" })),
            )
                .into_response(),

            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "message": "Unauthorized" })),
            )
                .into_response(),

            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(json!({ "message": "Forbidden" })),
            )
                .into_response(),

            AppError::Validation(e) => e.into_response(),

            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                Json(json!({ "message": msg })),
            )
                .into_response(),

            AppError::View(e) => {
                tracing::error!("View error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "message": "Internal server error" })),
                )
                    .into_response()
            }

            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "message": "Internal server error" })),
                )
                    .into_response()
            }

            AppError::Redis(e) => {
                tracing::error!("Redis error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "message": "Internal server error" })),
                )
                    .into_response()
            }

            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "Internal server error" })),
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    async fn status_of(err: AppError) -> StatusCode {
        err.into_response().status()
    }

    async fn body_json(err: AppError) -> serde_json::Value {
        let resp = err.into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ── Status codes (no DB / Redis needed) ──────────────────────────────────

    #[tokio::test]
    async fn not_found_is_404() {
        assert_eq!(status_of(AppError::NotFound).await, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unauthorized_is_401() {
        assert_eq!(status_of(AppError::Unauthorized).await, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn forbidden_is_403() {
        assert_eq!(status_of(AppError::Forbidden).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn conflict_is_409() {
        assert_eq!(
            status_of(AppError::Conflict("dup".to_string())).await,
            StatusCode::CONFLICT,
        );
    }

    #[tokio::test]
    async fn internal_is_500() {
        assert_eq!(status_of(AppError::Internal).await, StatusCode::INTERNAL_SERVER_ERROR);
    }

    // ── Response body shape ───────────────────────────────────────────────────

    #[tokio::test]
    async fn not_found_body_has_message_key() {
        let body = body_json(AppError::NotFound).await;
        assert!(body.get("message").is_some());
    }

    #[tokio::test]
    async fn conflict_body_contains_message_text() {
        let body = body_json(AppError::Conflict("Email already taken.".to_string())).await;
        assert_eq!(body["message"], "Email already taken.");
    }

    // ── DB integration test ───────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires PostgreSQL on localhost:5432"]
    async fn database_error_is_500() {
        let host = std::env::var("DB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
        let database = std::env::var("DB_DATABASE").unwrap_or_else(|_| "willowforge".to_string());
        let username = std::env::var("DB_USERNAME").unwrap_or_else(|_| "postgres".to_string());
        let password = std::env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".to_string());

        let url = format!("postgres://{}:{}@{}:{}/{}", username, password, host, port, database);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect(&url)
            .await
            .expect("DB connection failed");

        // Trigger a sqlx::Error by querying a nonexistent table.
        let result = sqlx::query("SELECT 1 FROM nonexistent_table_xyz")
            .fetch_one(&pool)
            .await;

        let err = AppError::Database(result.unwrap_err());
        assert_eq!(status_of(err).await, StatusCode::INTERNAL_SERVER_ERROR);
    }
}
