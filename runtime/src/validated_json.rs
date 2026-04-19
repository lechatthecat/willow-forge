use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use serde_json::json;
use validator::Validate;

pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ValidationError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|err| match err {
                JsonRejection::JsonDataError(e) => {
                    ValidationError::JsonError(e.to_string())
                }
                JsonRejection::JsonSyntaxError(e) => {
                    ValidationError::JsonError(e.to_string())
                }
                _ => ValidationError::JsonError("Invalid JSON".to_string()),
            })?;

        value.validate().map_err(ValidationError::ValidationError)?;

        Ok(ValidatedJson(value))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Validation failed")]
    ValidationError(validator::ValidationErrors),
    #[error("Invalid JSON: {0}")]
    JsonError(String),
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ValidationError::ValidationError(errors) => {
                let error_map = errors
                    .field_errors()
                    .iter()
                    .map(|(field, errors)| {
                        let messages: Vec<String> = errors
                            .iter()
                            .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
                            .collect();
                        (field.to_string(), messages)
                    })
                    .collect::<std::collections::HashMap<_, _>>();

                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    json!({
                        "message": "The given data was invalid.",
                        "errors": error_map
                    }),
                )
            }
            ValidationError::JsonError(msg) => (
                StatusCode::BAD_REQUEST,
                json!({
                    "message": "Invalid JSON",
                    "error": msg
                }),
            ),
        };

        (status, Json(message)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde::{Deserialize, Serialize};
    use validator::Validate;

    #[derive(Debug, Deserialize, Serialize, Validate)]
    struct Input {
        #[validate(length(min = 1, message = "name is required"))]
        name: String,
        #[validate(email(message = "invalid email"))]
        email: String,
    }

    async fn body_json(err: ValidationError) -> serde_json::Value {
        let resp = err.into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ── Status codes (no DB / Redis needed) ──────────────────────────────────

    #[tokio::test]
    async fn json_error_is_400() {
        let err = ValidationError::JsonError("unexpected token".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn validation_error_is_422() {
        let input = Input { name: String::new(), email: "not-an-email".to_string() };
        let errors = input.validate().unwrap_err();
        let err = ValidationError::ValidationError(errors);
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn validation_error_body_has_errors_key() {
        let input = Input { name: String::new(), email: "bad".to_string() };
        let errors = input.validate().unwrap_err();
        let body = body_json(ValidationError::ValidationError(errors)).await;
        assert!(body.get("errors").is_some());
        assert_eq!(body["message"], "The given data was invalid.");
    }

    #[tokio::test]
    async fn valid_input_passes_validation() {
        let input = Input { name: "Alice".to_string(), email: "alice@example.com".to_string() };
        assert!(input.validate().is_ok());
    }

    #[tokio::test]
    async fn empty_name_fails_validation() {
        let input = Input { name: String::new(), email: "alice@example.com".to_string() };
        let errors = input.validate().unwrap_err();
        let fields = errors.field_errors();
        assert!(fields.contains_key("name"));
    }

    #[tokio::test]
    async fn invalid_email_fails_validation() {
        let input = Input { name: "Alice".to_string(), email: "not-email".to_string() };
        let errors = input.validate().unwrap_err();
        let fields = errors.field_errors();
        assert!(fields.contains_key("email"));
    }

    // ── Router integration test (no DB / Redis needed) ───────────────────────

    #[tokio::test]
    async fn validated_json_extractor_accepts_valid_request() {
        use axum::{body::Body, http::Request, routing::post, Router};
        use tower::util::ServiceExt;

        async fn handler(ValidatedJson(input): ValidatedJson<Input>) -> String {
            input.name
        }

        let app = Router::new().route("/", post(handler));

        let req = Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"Bob","email":"bob@example.com"}"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&bytes[..], b"Bob");
    }

    #[tokio::test]
    async fn validated_json_extractor_rejects_invalid_input() {
        use axum::{body::Body, http::Request, routing::post, Router};
        use tower::util::ServiceExt;

        async fn handler(ValidatedJson(_): ValidatedJson<Input>) -> String {
            "ok".to_string()
        }

        let app = Router::new().route("/", post(handler));

        let req = Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"","email":"not-email"}"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn validated_json_extractor_rejects_malformed_json() {
        use axum::{body::Body, http::Request, routing::post, Router};
        use tower::util::ServiceExt;

        async fn handler(ValidatedJson(_): ValidatedJson<Input>) -> String {
            "ok".to_string()
        }

        let app = Router::new().route("/", post(handler));

        let req = Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", "application/json")
            .body(Body::from(r#"not json at all"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // ── DB integration test ───────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires PostgreSQL on localhost:5432"]
    async fn db_pool_connects_successfully() {
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

        let row: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 1);
    }
}
