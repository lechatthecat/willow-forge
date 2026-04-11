pub fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2024"

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
minijinja = {{ version = "2", features = ["loader"] }}
sqlx = {{ version = "0.8", features = ["postgres", "runtime-tokio-rustls", "chrono"] }}
chrono = {{ version = "0.4", features = ["serde"] }}
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

pub fn main_rs(name: &str) -> String {
    format!(
        r#"#[path = "../routes/api.rs"]
mod api;
#[path = "../routes/web.rs"]
mod web;
#[path = "../bootstrap/middleware.rs"]
mod middleware;

use anyhow::Result;
use {name}::bootstrap;
use tracing_subscriber::{{layer::SubscriberExt, util::SubscriberInitExt}};

#[tokio::main]
async fn main() -> Result<()> {{
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let app_state = bootstrap().await?;

    let app = middleware::global(
        middleware::api(api::routes())
            .merge(middleware::web(web::routes())),
    )
    .with_state(app_state);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("🌿 Willow server started on http://{{}}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}}
"#,
        name = name
    )
}

pub fn bootstrap_lib_rs() -> &'static str {
    r#"pub mod app_state;
pub mod context;
pub mod validated_json;
pub mod view;

#[path = "../app/errors.rs"]
pub mod app_errors;

#[path = "../app/Providers/AppServiceProvider.rs"]
mod app_service_provider;

pub use app_errors::AppError;
pub use app_state::{AppState, Config, Services, ViewEngine};
pub use context::Context;
pub use validated_json::ValidatedJson;
pub use view::view;

use anyhow::Result;
use minijinja::Environment;
use std::sync::Arc;

pub async fn bootstrap() -> Result<Arc<AppState>> {
    let config = Config {
        app_name: std::env::var("APP_NAME").unwrap_or_else(|_| "Willow".to_string()),
        app_env: std::env::var("APP_ENV").unwrap_or_else(|_| "local".to_string()),
        app_debug: std::env::var("APP_DEBUG")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
    };

    let views = build_view_engine()?;

    let db = app_service_provider::create_pool()?;
    let services = Services { db };

    Ok(Arc::new(AppState {
        config,
        services,
        views,
    }))
}

fn build_view_engine() -> Result<ViewEngine> {
    let mut env = Environment::new();
    let views_dir = std::path::PathBuf::from("resources/views");
    load_templates(&mut env, &views_dir, &views_dir)?;
    Ok(env)
}

fn load_templates(
    env: &mut Environment<'static>,
    base: &std::path::Path,
    dir: &std::path::Path,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            load_templates(env, base, &path)?;
        } else if path.extension().map(|e| e == "html").unwrap_or(false) {
            let rel = path.strip_prefix(base)?;
            let name = path_to_template_name(rel);
            let content = std::fs::read_to_string(&path)?;
            env.add_template_owned(name, content)
                .map_err(|e| anyhow::anyhow!("Template error in {:?}: {}", path, e))?;
        }
    }
    Ok(())
}

fn path_to_template_name(rel: &std::path::Path) -> String {
    let mut parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();

    if let Some(last) = parts.last_mut() {
        if let Some(stem) = last.strip_suffix(".jinja.html") {
            *last = stem.to_string();
        } else if let Some(stem) = last.strip_suffix(".html") {
            *last = stem.to_string();
        }
    }

    parts.join(".")
}
"#
}

pub fn app_errors_rs() -> &'static str {
    r#"use axum::{
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

            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "Internal server error" })),
            )
                .into_response(),
        }
    }
}
"#
}

pub fn app_state_rs() -> &'static str {
    r#"use minijinja::Environment;
use sqlx::PgPool;

pub type ViewEngine = Environment<'static>;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub services: Services,
    pub views: ViewEngine,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub app_name: String,
    pub app_env: String,
    pub app_debug: bool,
}

#[derive(Clone)]
pub struct Services {
    pub db: PgPool,
}
"#
}

pub fn view_rs() -> &'static str {
    r#"use axum::response::{Html, IntoResponse, Response};
use serde::Serialize;

use super::context::Context;

/// Render a view template and return an HTML response.
///
/// ```rust,ignore
/// use minijinja::context;
/// return view(&ctx, "welcome", context! { title => "Home" });
/// ```
pub fn view<S>(ctx: &Context, name: &str, data: S) -> Result<HtmlView, ViewError>
where
    S: Serialize,
{
    let tmpl = ctx
        .state
        .views
        .get_template(name)
        .map_err(|e| ViewError::NotFound(name.to_string(), e.to_string()))?;

    let rendered = tmpl
        .render(data)
        .map_err(|e| ViewError::RenderError(name.to_string(), e.to_string()))?;

    Ok(HtmlView(rendered))
}

pub struct HtmlView(String);

impl IntoResponse for HtmlView {
    fn into_response(self) -> Response {
        Html(self.0).into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error("View '{0}' not found: {1}")]
    NotFound(String, String),

    #[error("Failed to render view '{0}': {1}")]
    RenderError(String, String),
}

impl IntoResponse for ViewError {
    fn into_response(self) -> Response {
        use axum::http::StatusCode;
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
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
"#
}

pub fn app_service_provider() -> &'static str {
    r#"use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub fn create_pool() -> Result<PgPool> {
    let host     = std::env::var("DB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port     = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let database = std::env::var("DB_DATABASE")
        .context("DB_DATABASE environment variable is not set")?;
    let username = std::env::var("DB_USERNAME").unwrap_or_else(|_| "postgres".to_string());
    let password = std::env::var("DB_PASSWORD").unwrap_or_default();

    let url = format!(
        "postgres://{}:{}@{}:{}/{}",
        username, password, host, port, database
    );

    PgPoolOptions::new()
        .max_connections(10)
        .connect_lazy(&url)
        .with_context(|| format!(
            "Failed to configure Postgres pool for {}:{}/{}",
            host, port, database
        ))
}
"#
}

pub fn routes_api(name: &str) -> String {
    format!(
        r#"#[path = "../app/Http/Controllers/UserController.rs"]
mod user_controller;

#[path = "../app/Http/Controllers/StatusController.rs"]
mod status_controller;

use axum::{{routing::get, Router}};
use std::sync::Arc;

use {name}::AppState;

pub fn routes() -> Router<Arc<AppState>> {{
    Router::new()
        .route("/api/users", get(user_controller::index).post(user_controller::store))
        .route("/api/status", get(status_controller::index))
        .route("/api/users/mock", get(user_controller::mock))
}}
"#,
        name = name
    )
}

pub fn routes_web(name: &str) -> String {
    format!(
        r#"#[path = "../app/Http/Controllers/HomeController.rs"]
mod home_controller;

use axum::{{routing::get, Router}};
use std::sync::Arc;

use {name}::AppState;

pub fn routes() -> Router<Arc<AppState>> {{
    Router::new()
        .route("/", get(home_controller::index))
}}
"#,
        name = name
    )
}

pub fn home_controller(name: &str) -> String {
    format!(
        r#"use axum::response::IntoResponse;
use minijinja::context;

use {name}::{{AppError, Context}};
use {name}::view::view;

pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {{
    Ok(view(
        &ctx,
        "welcome",
        context! {{
            app_name => ctx.state.config.app_name.clone(),
            app_env  => ctx.state.config.app_env.clone(),
        }},
    )?)
}}
"#,
        name = name
    )
}

pub fn user_controller(name: &str) -> String {
    format!(
        r#"#[path = "../../Models/User.rs"]
mod user_model;

use axum::{{Json, response::IntoResponse, http::StatusCode}};
use serde::Deserialize;
use serde_json::json;
use validator::Validate;

use {name}::{{AppError, Context, ValidatedJson}};
use user_model::User;

// ============================================================
// Using AppError
//
// Handlers return Result<impl IntoResponse, AppError>.
// Return an error directly with Err(...) or propagate with ?.
//
// --- Returning errors explicitly ---
//
//   return Err(AppError::NotFound);                              // 404
//   return Err(AppError::Unauthorized);                         // 401
//   return Err(AppError::Forbidden);                            // 403
//   return Err(AppError::Conflict("Email already taken.".to_string())); // 409
//   return Err(AppError::Internal);                             // 500
//
// --- Automatic conversion via ? ---
//
//   sqlx::Error     → AppError::Database    (via #[from])
//   ViewError       → AppError::View        (via #[from])
//   ValidationError → AppError::Validation  (via #[from])
//
//   let users = sqlx::query_as::<_, User>(...).fetch_all(pool).await?;
//   // sqlx::Error is automatically converted to AppError::Database
//
// ============================================================

#[derive(Debug, Deserialize, Validate)]
pub struct StoreUserRequest {{
    #[validate(length(min = 1, max = 255, message = "Name is required and must be less than 255 characters"))]
    pub name: String,

    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}}

pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {{
    let pool = &ctx.state.services.db;

    let users = sqlx::query_as::<_, User>(
        "SELECT id, name, email, created_at FROM users ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    Ok((StatusCode::OK, Json(json!({{ "data": users }}))))
}}

pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<StoreUserRequest>,
) -> Result<impl IntoResponse, AppError> {{
    let pool = &ctx.state.services.db;

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (name, email, password)
         VALUES ($1, $2, $3)
         RETURNING id, name, email, created_at",
    )
    .bind(&req.name)
    .bind(&req.email)
    .bind(&req.password)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {{
        sqlx::Error::Database(ref db_err)
            if db_err.constraint() == Some("users_email_key") =>
        {{
            AppError::Conflict("Email already taken.".to_string())
        }}
        other => AppError::Database(other),
    }})?;

    Ok((StatusCode::CREATED, Json(json!({{ "ok": true, "data": user }}))))
}}

pub async fn mock(_ctx: Context) -> impl IntoResponse {{
    Json(json!({{
        "data": [
            {{ "id": 1, "name": "Alice", "email": "alice@example.com" }},
            {{ "id": 2, "name": "Bob",   "email": "bob@example.com"   }},
            {{ "id": 3, "name": "Carol", "email": "carol@example.com" }}
        ]
    }}))
}}
"#,
        name = name
    )
}

pub fn status_controller(name: &str) -> String {
    format!(
        r#"use axum::{{Json, response::IntoResponse}};
use serde_json::json;

use {name}::Context;

pub async fn index(ctx: Context) -> impl IntoResponse {{
    let connected = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&ctx.state.services.db),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false);

    Json(json!({{ "connected": connected }}))
}}
"#,
        name = name
    )
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

pub fn view_layout_app() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{ app_name }}{% endblock %}</title>
    <style>
        body { font-family: sans-serif; max-width: 800px; margin: 2rem auto; padding: 0 1rem; color: #333; }
        h1 { color: #2e7d32; }
        h2 { color: #555; font-size: 1rem; text-transform: uppercase; letter-spacing: 0.05em; margin-top: 2rem; }
        table { width: 100%; border-collapse: collapse; margin-top: 0.5rem; }
        th { text-align: left; padding: 0.4rem 0.75rem; background: #f5f5f5; border-bottom: 2px solid #ddd; font-size: 0.85rem; }
        td { padding: 0.4rem 0.75rem; border-bottom: 1px solid #eee; font-size: 0.9rem; }
        code { background: #f0f0f0; padding: 0.1em 0.35em; border-radius: 3px; font-size: 0.85em; }
        .badge { display: inline-block; font-size: 0.7rem; padding: 0.1em 0.5em; border-radius: 3px; font-weight: 600; vertical-align: middle; }
        .badge-db     { background: #f0f0f0; color: #555; border: 1px solid #ccc; }
        .badge-db-ok  { background: #d1e7dd; color: #0a3622; border: 1px solid #a3cfbb; }
        .badge-db-off { background: #f8d7da; color: #58151c; border: 1px solid #f1aeb5; }
    </style>
</head>
<body>
    {% block content %}{% endblock %}
</body>
</html>
"#
}

pub fn view_welcome() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}Welcome — {{ app_name }}{% endblock %}

{% block content %}
<h1>Welcome to {{ app_name }}</h1>
<p>Your Willow application is up and running.</p>
<ul>
    <li><strong>Framework:</strong> Willow</li>
    <li><strong>Environment:</strong> {{ app_env }}</li>
    <li><strong>View engine:</strong> MiniJinja</li>
    <li><strong>Database:</strong> <span id="db-status">checking...</span></li>
</ul>

<h2>Available Routes</h2>
<table>
    <thead>
        <tr><th>Method</th><th>URL</th><th>Description</th></tr>
    </thead>
    <tbody>
        <tr><td><code>GET</code></td><td><code><a href="/">/</a></code></td><td>Welcome page</td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/api/users">/api/users</a></code></td><td>List all users <span class="badge badge-db" id="db-badge-1">DB</span></td></tr>
        <tr><td><code>POST</code></td><td><code>/api/users</code></td><td>Create a new user <span class="badge badge-db" id="db-badge-2">DB</span></td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/api/users/mock">/api/users/mock</a></code></td><td>List users (mock JSON, no DB)</td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/api/status">/api/status</a></code></td><td>Database connection status</td></tr>
    </tbody>
</table>

<script>
    fetch('/api/status')
        .then(r => r.json())
        .then(data => {
            const ok = data.connected;
            document.getElementById('db-status').textContent = ok ? 'Connected ✓' : 'Not connected ✗';

            const cls = ok ? 'badge-db-ok' : 'badge-db-off';
            const label = ok ? 'DB — you are connected' : 'DB — you are not connected';
            ['db-badge-1', 'db-badge-2'].forEach(id => {
                const el = document.getElementById(id);
                el.className = 'badge ' + cls;
                el.textContent = label;
            });
        })
        .catch(() => {
            document.getElementById('db-status').textContent = 'Not connected ✗';
            ['db-badge-1', 'db-badge-2'].forEach(id => {
                const el = document.getElementById(id);
                el.className = 'badge badge-db-off';
                el.textContent = 'DB — you are not connected';
            });
        });
</script>
{% endblock %}
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

pub fn user_model_rs() -> &'static str {
    r#"use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
}
"#
}

pub fn initial_migration_up_sql() -> &'static str {
    r#"CREATE TABLE IF NOT EXISTS users (
    id         SERIAL PRIMARY KEY,
    name       VARCHAR(255)  NOT NULL,
    email      VARCHAR(255)  NOT NULL UNIQUE,
    password   VARCHAR(255)  NOT NULL,
    created_at TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);
"#
}

pub fn initial_migration_down_sql() -> &'static str {
    "DROP TABLE IF EXISTS users;\n"
}

pub fn bootstrap_middleware_rs(name: &str) -> String {
    format!(
        r#"#[path = "../app/Http/Middleware/LogRequest.rs"]
mod log_request;

use axum::{{middleware, Router}};
use std::sync::Arc;

use {name}::AppState;

/// Global middleware — runs on every request.
/// Add new middleware here to apply it across the entire application.
pub fn global(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    router
        .layer(middleware::from_fn(log_request::handle))
}}

/// Web middleware — runs only on HTML routes (routes/web.rs).
pub fn web(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    router
    // .layer(middleware::from_fn(csrf::handle))
}}

/// API middleware — runs only on API routes (routes/api.rs).
pub fn api(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    router
    // .layer(middleware::from_fn(auth::handle))
}}
"#,
        name = name,
    )
}

pub fn middleware_log_request_rs() -> &'static str {
    r#"use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;

pub async fn handle(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    let response = next.run(request).await;

    tracing::info!(
        "{} {} → {} ({:?})",
        method,
        uri,
        response.status(),
        start.elapsed()
    );

    response
}
"#
}

pub fn make_middleware_template(name: &str) -> String {
    let snake = pascal_to_snake(name);
    format!(
        r#"use axum::{{
    extract::Request,
    middleware::Next,
    response::Response,
}};

/// {name} middleware
///
/// To register this middleware, add it to bootstrap/middleware.rs:
///
///   #[path = "../app/Http/Middleware/{name}.rs"]
///   mod {snake};
///
///   // In global(), api(), or web():
///   router.layer(axum::middleware::from_fn({snake}::handle))
///
pub async fn handle(request: Request, next: Next) -> Response {{
    // Before the handler runs
    let response = next.run(request).await;
    // After the handler runs
    response
}}
"#,
        name = name,
        snake = snake,
    )
}

fn pascal_to_snake(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_lowercase().next().unwrap());
    }
    result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_toml_contains_package_name() {
        let out = cargo_toml("my-app");
        assert!(out.contains("name = \"my-app\""));
    }

    #[test]
    fn main_rs_uses_crate_name_in_import() {
        let out = main_rs("my_app");
        assert!(out.contains("use my_app::bootstrap"));
    }

    #[test]
    fn routes_api_uses_crate_name() {
        let out = routes_api("my_app");
        assert!(out.contains("use my_app::AppState"));
    }

    #[test]
    fn routes_web_uses_crate_name() {
        let out = routes_web("my_app");
        assert!(out.contains("use my_app::AppState"));
    }

    #[test]
    fn home_controller_uses_crate_name() {
        let out = home_controller("my_app");
        assert!(out.contains("use my_app::{AppError, Context}"));
        assert!(out.contains("Result<impl IntoResponse, AppError>"));
    }

    #[test]
    fn user_controller_uses_crate_name() {
        let out = user_controller("my_app");
        assert!(out.contains("use my_app::{AppError, Context, ValidatedJson}"));
        assert!(out.contains("Result<impl IntoResponse, AppError>"));
    }
}
