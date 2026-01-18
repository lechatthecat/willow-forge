pub fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = {{ version = "1", features = ["full"] }}
tower = "0.4"
tower-http = {{ version = "0.5", features = ["cors", "trace"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
validator = {{ version = "0.18", features = ["derive"] }}
dotenvy = "0.15"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

[lib]
path = "bootstrap/lib.rs"

[[bin]]
name = "{}"
path = "src/main.rs"
"#,
        name, name
    )
}

pub fn env_file() -> &'static str {
    r#"APP_NAME=Willow
APP_ENV=local
APP_DEBUG=true
APP_URL=http://localhost:3000

DB_CONNECTION=postgres
DB_HOST=127.0.0.1
DB_PORT=5432
DB_DATABASE=willow
DB_USERNAME=postgres
DB_PASSWORD=
"#
}

pub fn main_rs() -> &'static str {
    r#"#[path = "../routes/api.rs"]
mod api;
#[path = "../routes/web.rs"]
mod web;

use anyhow::Result;
use demo_app::bootstrap;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Bootstrap the application
    let app_state = bootstrap()?;

    // Build router
    let app = api::routes()
        .merge(web::routes())
        .with_state(app_state);

    // Start server
    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("🌿 Willow server started on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
"#
}

pub fn bootstrap_app() -> &'static str {
    r#"pub mod app_state;
pub mod context;
pub mod validated_json;

use anyhow::Result;
use std::sync::Arc;

pub use app_state::{AppState, Config, Services};
pub use context::Context;
pub use validated_json::ValidatedJson;

pub fn bootstrap() -> Result<Arc<AppState>> {
    // Load configuration
    let config = Arc::new(Config {
        app_name: std::env::var("APP_NAME").unwrap_or_else(|_| "Willow".to_string()),
        app_env: std::env::var("APP_ENV").unwrap_or_else(|_| "local".to_string()),
        app_debug: std::env::var("APP_DEBUG")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
    });

    // Build services using providers
    let services = app::Providers::AppServiceProvider::register();

    let app_state = AppState {
        config,
        services: Arc::new(services),
    };

    Ok(Arc::new(app_state))
}
"#
}

pub fn bootstrap_lib_rs() -> &'static str {
    r#"pub mod app_state;
pub mod context;
pub mod validated_json;

pub use app_state::{AppState, Config, Services};
pub use context::Context;
pub use validated_json::ValidatedJson;

use anyhow::Result;
use std::sync::Arc;

pub fn bootstrap() -> Result<Arc<AppState>> {
    // Load configuration
    let config = Arc::new(Config {
        app_name: std::env::var("APP_NAME").unwrap_or_else(|_| "Willow".to_string()),
        app_env: std::env::var("APP_ENV").unwrap_or_else(|_| "local".to_string()),
        app_debug: std::env::var("APP_DEBUG")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
    });

    // Build services
    let services = Services {
        // Initialize services here
    };

    let app_state = AppState {
        config,
        services: Arc::new(services),
    };

    Ok(Arc::new(app_state))
}
"#
}

pub fn app_state_rs() -> &'static str {
    r#"use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub services: Arc<Services>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub app_name: String,
    pub app_env: String,
    pub app_debug: bool,
}

#[derive(Clone)]
pub struct Services {
    // Add your services here
    // Example: pub db: Arc<DatabasePool>,
}
"#
}

pub fn context_rs() -> &'static str {
    r#"use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use std::sync::Arc;

use super::app_state::AppState;

#[derive(Clone)]
pub struct Context {
    pub state: Arc<AppState>,
}

#[async_trait]
impl<S> FromRequestParts<S> for Context
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let app_state = Arc::<AppState>::from_ref(state);

        Ok(Context { state: app_state })
    }
}
"#
}

pub fn validated_json_rs() -> &'static str {
    r#"use axum::{
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

pub enum ValidationError {
    ValidationError(validator::ValidationErrors),
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
                        "message": "Validation failed",
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
"#
}

pub fn app_service_provider() -> &'static str {
    r#"use bootstrap::Services;

pub fn register() -> Services {
    // Initialize your services here
    // Example: let db = create_database_pool();

    Services {
        // Add your initialized services
    }
}
"#
}

pub fn routes_api() -> &'static str {
    r#"use axum::{routing::post, Router, Json, http::StatusCode, response::IntoResponse};
use std::sync::Arc;
use serde_json::json;
use serde::Deserialize;
use validator::Validate;

// Re-export from the demo-app crate (which is the lib bootstrap)
use demo_app::{AppState, Context, ValidatedJson};

#[derive(Debug, Deserialize, Validate)]
struct StoreUserRequest {
    #[validate(length(min = 1, max = 255, message = "Name is required and must be less than 255 characters"))]
    name: String,

    #[validate(email(message = "Must be a valid email address"))]
    email: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    password: String,
}

async fn store_user(
    _ctx: Context,
    ValidatedJson(req): ValidatedJson<StoreUserRequest>,
) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "data": {
                "name": req.name,
                "email": req.email
            }
        }))
    )
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/users", post(store_user))
}
"#
}

pub fn routes_web() -> &'static str {
    r#"use axum::{routing::get, Router, Json};
use std::sync::Arc;
use serde_json::json;

use demo_app::AppState;

async fn index() -> Json<serde_json::Value> {
    Json(json!({
        "message": "Welcome to Willow Framework",
        "version": "0.1.0"
    }))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(index))
}
"#
}

pub fn user_controller() -> &'static str {
    r#"use axum::{Json, response::IntoResponse, http::StatusCode};
use serde_json::json;

use bootstrap::{Context, ValidatedJson};
use app::Http::Requests::StoreUserRequest;

pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<StoreUserRequest>,
) -> impl IntoResponse {
    // Here you would typically save to database
    // For now, just return success

    (
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "data": {
                "name": req.name,
                "email": req.email
            }
        }))
    )
}
"#
}

pub fn store_user_request() -> &'static str {
    r#"use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct StoreUserRequest {
    #[validate(length(min = 1, max = 255, message = "Name is required and must be less than 255 characters"))]
    pub name: String,

    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}
"#
}

pub fn config_app() -> &'static str {
    r#"[app]
name = "Willow"
env = "local"
debug = true
url = "http://localhost:3000"
"#
}

pub fn config_database() -> &'static str {
    r#"[database]
connection = "postgres"
host = "127.0.0.1"
port = 5432
database = "willow"
username = "postgres"
password = ""
"#
}

pub fn gitignore() -> &'static str {
    r#"/target
.env
.env.*
!.env.example
*.log
/storage/logs/*
/storage/cache/*
.DS_Store
Cargo.lock
"#
}
