pub fn cargo_toml(name: &str) -> String {
    format!(
        r#"[workspace]

[package]
name = "{}"
version = "0.1.0"
edition = "2024"

[dependencies]
willow-forge-runtime = {{ path = "/home/shu/Documents/willow/runtime" }}
axum = "0.8.9"
tokio = {{ version = "1", features = ["full"] }}
tower = "0.5.3"
tower-http = {{ version = "0.6.8", features = ["cors", "trace"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
validator = {{ version = "0.20.0", features = ["derive"] }}
dotenvy = "0.15"
anyhow = "1"
minijinja = {{ version = "2", features = ["loader"] }}
sqlx = {{ version = "0.8", features = ["postgres", "runtime-tokio-rustls", "chrono"] }}
redis = {{ version = "1.2.0", features = ["tokio-comp", "cluster-async"] }}
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
    r#"APP_NAME="Willow Forge"
APP_ENV=local
APP_DEBUG=true
APP_URL=http://localhost:3000

DB_CONNECTION=postgres
DB_HOST=127.0.0.1
DB_PORT=5432
DB_DATABASE=willowforge
DB_USERNAME=postgres
DB_PASSWORD=postgres

REDIS_CLUSTER_NODES=redis://127.0.0.1:7001,redis://127.0.0.1:7002,redis://127.0.0.1:7003

SESSION_LIFETIME=7200
SESSION_SECURE=false

JWT_SECRET=change-me-in-production
JWT_EXPIRY=3600
"#
}

pub fn main_rs(name: &str) -> String {
    format!(
        r#"mod app;
mod middleware;
mod routes;

use anyhow::Result;
use std::sync::Arc;
use {name}::{{bootstrap, AppError}};
use tracing_subscriber::{{layer::SubscriberExt, util::SubscriberInitExt}};

async fn not_found() -> impl axum::response::IntoResponse {{
    AppError::NotFound
}}

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
        Arc::clone(&app_state),
        middleware::api(routes::api::routes())
            .merge(middleware::web(routes::web::routes()))
            .fallback(not_found),
    )
    .layer(axum::middleware::from_fn_with_state(
        Arc::clone(&app_state),
        app::Exceptions::Handler::render,
    ))
    .with_state(app_state);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("🌿 Willow Forge server started on http://{{}}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}}
"#,
        name = name
    )
}

pub fn bootstrap_lib_rs() -> &'static str {
    r#"pub use willow_forge_runtime::{
    AppError, AppState, Auth, AuthUser, Cache, Config, Context, Hash, Jwt, JwtUser,
    RedisCluster, RedisConfig, Services, Session, ValidatedJson, ViewEngine,
    authenticate, session_middleware, view,
};

mod app_service_provider;

use anyhow::Result;
use minijinja::Environment;
use std::sync::Arc;

pub async fn bootstrap() -> Result<Arc<AppState>> {
    let redis_nodes: Vec<String> = std::env::var("REDIS_CLUSTER_NODES")
        .unwrap_or_else(|_| {
            "redis://127.0.0.1:7001,redis://127.0.0.1:7002,redis://127.0.0.1:7003".to_string()
        })
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let config = Config {
        app_name: std::env::var("APP_NAME").unwrap_or_else(|_| "Willow Forge".to_string()),
        app_env:  std::env::var("APP_ENV").unwrap_or_else(|_| "local".to_string()),
        app_debug: std::env::var("APP_DEBUG")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
        redis: RedisConfig { nodes: redis_nodes.clone() },
    };

    let views = build_view_engine()?;

    let db    = app_service_provider::create_pool()?;
    let redis = app_service_provider::create_redis_cluster(&redis_nodes)?;

    let services = Services { db, redis };

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

pub fn exception_handler_rs(name: &str) -> String {
    format!(
        r#"use axum::{{
    extract::{{Request, State}},
    middleware::Next,
    response::{{Html, IntoResponse, Response}},
}};
use minijinja::context;
use std::sync::Arc;

use {name}::AppState;

/// Equivalent to Laravel's `$request->expectsJson()`.
///
/// Returns true if:
/// - Accept header contains `application/json`, `/json`, or `+json`  (wantsJson)
/// - OR Content-Type: application/json (client is sending JSON → expects JSON back)
/// - OR the request is an AJAX call (X-Requested-With: XMLHttpRequest) with Accept: */* or absent
fn expects_json(request: &Request) -> bool {{
    let accept = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // wantsJson()
    let wants_json = accept.contains("application/json")
        || accept.contains("/json")
        || accept.contains("+json");

    // If the request body is JSON, the client is an API client and expects JSON errors
    let sends_json = request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("application/json"))
        .unwrap_or(false);

    // ajax() — X-Requested-With: XMLHttpRequest
    let is_ajax = request
        .headers()
        .get("x-requested-with")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("xmlhttprequest"))
        .unwrap_or(false);

    // acceptsAnyContentType() — Accept: */* or header absent
    let accepts_any = accept.is_empty() || accept.contains("*/*");

    wants_json || sends_json || (is_ajax && accepts_any)
}}

/// Exception handler — intercepts error responses and renders HTML error views when available.
///
/// How it works:
/// - Runs on every response (as the outermost layer in main.rs)
/// - If `expects_json()` is true (Laravel-equivalent), passes through as-is
/// - Otherwise, if the status is 4xx/5xx, looks for resources/views/errors/{{code}}.jinja.html
/// - If found, replaces the response with the rendered HTML view
/// - If not found, passes through the original response unchanged
///
/// To add a custom error view, create resources/views/errors/404.jinja.html etc.
/// To add shared logic (logging, alerting), add it inside this function.
/// To force JSON for specific paths (like Laravel's shouldRenderJsonWhen), modify expects_json().
pub async fn render(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {{
    let json_expected = expects_json(&request);

    let response = next.run(request).await;

    // If the client expects JSON, skip HTML error view rendering
    if json_expected {{
        return response;
    }}

    let status = response.status();

    if !status.is_client_error() && !status.is_server_error() {{
        return response;
    }}

    let code = status.as_u16();
    let template_name = format!("errors.{{}}", code);

    let data = context! {{
        code     => code,
        message  => status.canonical_reason().unwrap_or("Error"),
        app_name => state.config.app_name.clone(),
        app_env  => state.config.app_env.clone(),
    }};

    let tmpl = state.views.get_template(&template_name)
        .or_else(|_| state.views.get_template("errors.generic"));

    if let Ok(tmpl) = tmpl {{
        if let Ok(html) = tmpl.render(data) {{
            return (status, Html(html)).into_response();
        }}
    }}

    response
}}
"#,
        name = name
    )
}

pub fn view_error_404_html() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}404 — Not Found | {{ app_name }}{% endblock %}

{% block content %}
<h1>{{ code }}</h1>
<p>{{ message }}</p>
<p><a href="/">← Back to home</a></p>
{% endblock %}
"#
}

pub fn view_error_500_html() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}500 — Server Error | {{ app_name }}{% endblock %}

{% block content %}
<h1>{{ code }}</h1>
<p>{{ message }}</p>
{% endblock %}
"#
}

pub fn view_error_generic_html() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}{{ code }} — {{ message }} | {{ app_name }}{% endblock %}

{% block content %}
<h1>{{ code }}</h1>
<p>{{ message }}</p>
{% endblock %}
"#
}

pub fn app_service_provider() -> &'static str {
    r#"use anyhow::{Context, Result};
use redis::cluster::ClusterClient;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;
use std::sync::Arc;

pub fn create_pool() -> Result<PgPool> {
    let host     = std::env::var("DB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string())
        .parse().unwrap_or(5432);
    let database = std::env::var("DB_DATABASE").unwrap_or_default();
    let username = std::env::var("DB_USERNAME").unwrap_or_else(|_| "postgres".to_string());
    let password = std::env::var("DB_PASSWORD").unwrap_or_default();

    let opts = PgConnectOptions::new()
        .host(&host)
        .port(port)
        .database(&database)
        .username(&username)
        .password(&password)
        .ssl_mode(PgSslMode::Disable);

    Ok(PgPoolOptions::new()
        .max_connections(10)
        .connect_lazy_with(opts))
}

/// Build a Redis cluster client from a list of node URLs.
///
/// Only validates config syntax — no actual connection is made here.
/// If the cluster is down the app still starts; Redis endpoints will fail gracefully.
pub fn create_redis_cluster(nodes: &[String]) -> Result<Arc<ClusterClient>> {
    let client = ClusterClient::new(nodes.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .with_context(|| format!("Failed to configure Redis cluster client for nodes: {:?}", nodes))?;
    Ok(Arc::new(client))
}
"#
}

pub fn routes_api(name: &str) -> String {
    format!(
        r#"use axum::{{routing::get, Router}};
use std::sync::Arc;

use {name}::AppState;
use crate::app::Http::Controllers::{{UserController, StatusController}};

pub fn routes() -> Router<Arc<AppState>> {{
    Router::new()
        .route("/api/users", get(UserController::index).post(UserController::store))
        .route("/api/status", get(StatusController::index))
        .route("/api/users/mock", get(UserController::mock))
}}
"#,
        name = name
    )
}

pub fn routes_web(name: &str) -> String {
    format!(
        r#"use axum::{{routing::get, Router}};
use std::sync::Arc;

use {name}::AppState;
use crate::app::Http::Controllers::HomeController;

pub fn routes() -> Router<Arc<AppState>> {{
    Router::new()
        .route("/", get(HomeController::index))
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
        r#"use axum::{{Json, response::IntoResponse, http::StatusCode}};
use serde::Deserialize;
use serde_json::json;
use validator::Validate;

use {name}::{{AppError, Cache, Context, ValidatedJson}};
use crate::app::Models::User::User;

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
//   redis::RedisError → AppError::Redis     (via #[from])
//
//   let users = sqlx::query_as::<_, User>(...).fetch_all(pool).await?;
//   // sqlx::Error is automatically converted to AppError::Database
//
// --- Using the Cache facade (Redis) ---
//
//   // Cache a DB query for 60 seconds
//   let users = Cache::remember(&ctx, "users.all", Duration::from_secs(60), || async {{
//       sqlx::query_as::<_, User>("SELECT ...").fetch_all(pool).await.map_err(AppError::from)
//   }}).await?;
//
//   // Store a value
//   Cache::put(&ctx, "my.key", &value, Duration::from_secs(60)).await?;
//
//   // Read a value
//   let val: Option<String> = Cache::get(&ctx, "my.key").await?;
//
//   // Delete a value
//   Cache::forget(&ctx, "my.key").await?;
//
// ============================================================

use std::time::Duration;

#[derive(Debug, Deserialize, Validate)]
pub struct StoreUserRequest {{
    #[validate(length(min = 1, max = 255, message = "Name is required and must be less than 255 characters"))]
    pub name: String,

    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}}

/// GET /api/users
/// Returns all users, cached in Redis for 60 seconds.
pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {{
    let pool = ctx.state.services.db.clone();

    let users = Cache::remember(&ctx, "users.all", Duration::from_secs(60), || async move {{
        sqlx::query_as::<_, User>(
            "SELECT id, name, email, password, created_at FROM users ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .map_err(AppError::from)
    }})
    .await?;

    Ok((StatusCode::OK, Json(json!({{ "data": users }}))))
}}

/// POST /api/users
/// Creates a user in the DB and invalidates the users.all cache.
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<StoreUserRequest>,
) -> Result<impl IntoResponse, AppError> {{
    let pool = &ctx.state.services.db;

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (name, email, password)
         VALUES ($1, $2, $3)
         RETURNING id, name, email, password, created_at",
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

    // Invalidate the cached user list so the next GET reflects the new user.
    Cache::forget(&ctx, "users.all").await?;

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
    // Both checks run concurrently — neither blocks the other.
    let (db, redis) = tokio::join!(
        check_db(&ctx),
        check_redis(&ctx),
    );

    Json(json!({{ "db": db, "redis": redis }}))
}}

async fn check_db(ctx: &Context) -> bool {{
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&ctx.state.services.db),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}}

async fn check_redis(ctx: &Context) -> bool {{
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        async {{
            match ctx.state.services.redis.get_async_connection().await {{
                Ok(mut conn) => redis::cmd("PING").query_async::<String>(&mut conn).await.is_ok(),
                Err(_) => false,
            }}
        }},
    )
    .await
    .unwrap_or(false)
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
        .auth-form { max-width: 400px; margin: 4rem auto; }
        .auth-form div { margin-bottom: 1rem; }
        .auth-form label { display: block; font-size: 0.9rem; margin-bottom: 0.25rem; font-weight: 600; }
        .auth-form input { width: 100%; padding: 0.5rem 0.75rem; border: 1px solid #ccc; border-radius: 4px; font-size: 1rem; box-sizing: border-box; }
        .auth-form button { width: 100%; padding: 0.6rem; background: #2e7d32; color: #fff; border: none; border-radius: 4px; font-size: 1rem; cursor: pointer; }
        .auth-form button:hover { background: #1b5e20; }
        .alert { padding: 0.75rem 1rem; border-radius: 4px; margin-bottom: 1rem; font-size: 0.9rem; }
        .alert-error { background: #f8d7da; color: #58151c; border: 1px solid #f1aeb5; }
        .alert-success { background: #d1e7dd; color: #0a3622; border: 1px solid #a3cfbb; }
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
<p>Your Willow Forge application is up and running.</p>
<ul>
    <li><strong>Framework:</strong> Willow Forge</li>
    <li><strong>Environment:</strong> {{ app_env }}</li>
    <li><strong>View engine:</strong> MiniJinja</li>
    <li><strong>Database:</strong> <span id="db-status">checking...</span></li>
    <li><strong>Redis:</strong> <span id="redis-status">checking...</span></li>
</ul>

<h2>Getting Started</h2>
<p>Start the database and Redis cluster with Docker:</p>
<pre><code>docker compose -f docker/docker-compose.yml up -d --build</code></pre>
<p>Run database migrations (safe to run while the server is up &mdash; it only connects to PostgreSQL, not port 3000):</p>
<pre><code>willow-forge migrate</code></pre>

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
        <tr><td><code>GET</code></td><td><code><a href="/api/status">/api/status</a></code></td><td>Database and Redis connection status</td></tr>
    </tbody>
</table>

<h2>Auth Setup</h2>
<table>
    <thead>
        <tr><th>Command</th><th>Type</th><th>Response</th></tr>
    </thead>
    <tbody>
        <tr><td><code>willow-forge make:auth</code></td><td>HTML form (web routes)</td><td>Redirect, renders views</td></tr>
        <tr><td><code>willow-forge make:auth --api</code></td><td>REST API (api routes)</td><td>JSON responses, no views</td></tr>
    </tbody>
</table>
<p>Scaffold HTML form-based auth (session login/register/logout with views):</p>
<pre><code>willow-forge make:auth</code></pre>
<p>Routes are injected automatically into <code>src/routes/web.rs</code>. Restart the server to activate them.</p>
<p>Restart the server (<code>cargo run</code>) and the following routes will be available:</p>
<table>
    <thead>
        <tr><th>Method</th><th>URL</th><th>Description</th></tr>
    </thead>
    <tbody>
        <tr><td><code>GET</code></td><td><code><a href="/login">/login</a></code></td><td>Login form</td></tr>
        <tr><td><code>POST</code></td><td><code>/login</code></td><td>Submit login</td></tr>
        <tr><td><code>POST</code></td><td><code>/logout</code></td><td>Logout</td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/register">/register</a></code></td><td>Registration form</td></tr>
        <tr><td><code>POST</code></td><td><code>/register</code></td><td>Submit registration</td></tr>
    </tbody>
</table>

<h2>Auth curl Examples</h2>
<h3>Register</h3>
<pre><code>curl -s -c cookies.txt -X POST http://localhost:3000/register \
  -d 'name=Alice&amp;email=alice@example.com&amp;password=secret123'</code></pre>

<h3>Login</h3>
<pre><code>curl -s -c cookies.txt -b cookies.txt -X POST http://localhost:3000/login \
  -d 'email=alice@example.com&amp;password=secret123'</code></pre>

<h3>Logout</h3>
<pre><code>curl -s -c cookies.txt -b cookies.txt -X POST http://localhost:3000/logout</code></pre>

<h2>API Examples</h2>

<h3>POST /api/users — success</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d '{"name":"Alice","email":"alice@example.com","password":"secret123"}' | jq</code></pre>

<h3>POST /api/users — validation error (422)</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d '{"name":"","email":"not-an-email","password":"short"}' | jq</code></pre>

<h3>POST /api/users — malformed JSON (400)</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d 'invalid' | jq</code></pre>

<h2>Docker Hints</h2>
<h3>Get inside a container</h3>
<pre><code>docker exec -it redis-node-1 sh</code></pre>

<h3>Logs</h3>
<pre><code>docker logs --tail 50 --follow --timestamps postgres-db
docker logs --tail 50 --follow --timestamps redis-node-1</code></pre>

<h3>List containers</h3>
<pre><code>docker ps -a</code></pre>

<h3>Stop containers</h3>
<pre><code>docker compose -f docker/docker-compose.yml down</code></pre>

<h3>Check volumes</h3>
<pre><code>docker volume ls
docker volume inspect &lt;volume-name&gt;</code></pre>

<h3>Other commands</h3>
<p>Flush Redis cache:</p>
<pre><code>docker exec -it redis-node-1 redis-cli -p 7001 FLUSHALL</code></pre>

<p>If Redis cluster init fails with <code>[ERR] Node is not empty</code>, reset all nodes before re-initializing:</p>
<p>bash / Git Bash:</p>
<pre><code>for port in 7001 7002 7003 7004 7005 7006; do
  docker exec redis-node-$(( port - 7000 )) redis-cli -p $port FLUSHALL
  docker exec redis-node-$(( port - 7000 )) redis-cli -p $port CLUSTER RESET
done
docker compose -f docker/docker-compose.yml restart redis-cluster-init</code></pre>
<p>PowerShell:</p>
<pre><code>foreach ($port in 7001, 7002, 7003, 7004, 7005, 7006) { $node = $port - 7000; docker exec redis-node-$node redis-cli -p $port FLUSHALL; docker exec redis-node-$node redis-cli -p $port CLUSTER RESET }; docker compose -f docker/docker-compose.yml restart redis-cluster-init</code></pre>


<script>
    fetch('/api/status')
        .then(r => r.json())
        .then(data => {
            // DB status
            const dbOk = data.db;
            document.getElementById('db-status').textContent = dbOk ? 'Connected ✓' : 'Not connected ✗';
            const dbCls = dbOk ? 'badge-db-ok' : 'badge-db-off';
            const dbLabel = dbOk ? 'DB — connected' : 'DB — not connected';
            ['db-badge-1', 'db-badge-2'].forEach(id => {
                const el = document.getElementById(id);
                el.className = 'badge ' + dbCls;
                el.textContent = dbLabel;
            });

            // Redis status
            const redisOk = data.redis;
            const redisEl = document.getElementById('redis-status');
            redisEl.textContent = redisOk ? 'Connected ✓' : 'Not connected ✗';
            redisEl.style.color = redisOk ? '#0a3622' : '#58151c';
        })
        .catch(() => {
            document.getElementById('db-status').textContent = 'Not connected ✗';
            ['db-badge-1', 'db-badge-2'].forEach(id => {
                const el = document.getElementById(id);
                el.className = 'badge badge-db-off';
                el.textContent = 'DB — not connected';
            });
            const redisEl = document.getElementById('redis-status');
            redisEl.textContent = 'Not connected ✗';
            redisEl.style.color = '#58151c';
        });
</script>
{% endblock %}
"#
}

pub fn config_app() -> &'static str {
    r#"[app]
name = "Willow Forge"
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
database = "willowforge"
username = "postgres"
password = ""
"#
}

pub fn src_app_mod_rs() -> &'static str {
    "pub mod Http;\npub mod Models;\npub mod Exceptions;\n"
}

pub fn src_app_http_mod_rs() -> &'static str {
    "pub mod Controllers;\npub mod Middleware;\npub mod Requests;\n"
}

pub fn src_app_http_controllers_mod_rs() -> &'static str {
    "pub mod HomeController;\npub mod UserController;\npub mod StatusController;\n"
}

pub fn src_app_http_middleware_mod_rs() -> &'static str {
    "pub mod LogRequest;\n"
}

pub fn src_app_http_requests_mod_rs() -> &'static str {
    ""
}

pub fn src_app_models_mod_rs() -> &'static str {
    "pub mod User;\n"
}

pub fn src_app_exceptions_mod_rs() -> &'static str {
    "pub mod Handler;\n"
}

pub fn src_routes_mod_rs() -> &'static str {
    "pub mod web;\npub mod api;\n"
}

pub fn models_mod_rs() -> &'static str {
    "pub mod User;\n"
}

pub fn user_model_rs() -> &'static str {
    r#"use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub async fn find_by_email(db: &PgPool, email: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM users WHERE email = $1 LIMIT 1")
            .bind(email)
            .fetch_optional(db)
            .await
    }
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
        r#"use crate::app::Http::Middleware::LogRequest;

use axum::{{extract::Request, middleware, middleware::Next, Router}};
use std::sync::Arc;

use {name}::{{AppState, session_middleware}};

/// Global middleware — runs on every request.
pub fn global(state: Arc<AppState>, router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    let sess = Arc::clone(&state);
    router
        .layer(middleware::from_fn(LogRequest::handle))
        .layer(middleware::from_fn(move |req: Request, next: Next| {{
            let s = Arc::clone(&sess);
            async move {{ session_middleware(s, req, next).await }}
        }}))
}}

/// Web middleware — runs only on HTML routes (src/routes/web.rs).
pub fn web(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    router
    // Protect all web routes with auth:
    // .layer(middleware::from_fn({name}::authenticate))
}}

/// API middleware — runs only on API routes (src/routes/api.rs).
pub fn api(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    router
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
/// To register this middleware, add it to src/middleware.rs:
///
///   use crate::app::Http::Middleware::{name};
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

// ── Auth scaffolding templates ────────────────────────────────────────────────

pub fn config_auth() -> &'static str {
    r#"[auth]
guard = "web"
redirect = "/login"
session_lifetime = 7200
session_cookie = "willow_session"
session_secure = false
"#
}

pub fn make_auth_login_request() -> &'static str {
    r#"use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,

    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}
"#
}

pub fn make_auth_register_request() -> &'static str {
    r#"use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 1, max = 255, message = "Name is required"))]
    pub name: String,

    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}
"#
}

pub fn make_auth_login_controller(name: &str) -> String {
    format!(
        r#"use axum::{{
    extract::Form,
    response::{{IntoResponse, Redirect}},
}};

use {name}::{{AppError, Auth, Context, Hash, Session, view}};
use crate::app::Http::Requests::LoginRequest::LoginRequest;

/// GET /login
pub async fn show(ctx: Context, session: Session) -> Result<impl IntoResponse, AppError> {{
    let error: Option<String> = session.get("flash_error");
    session.forget("flash_error");
    Ok(view(&ctx, "auth.login", minijinja::context! {{ flash_error => error }})?)
}}

/// POST /login
pub async fn store(
    ctx: Context,
    session: Session,
    Form(req): Form<LoginRequest>,
) -> impl IntoResponse {{
    use validator::Validate;
    if let Err(errors) = req.validate() {{
        let msg = errors.field_errors()
            .values()
            .flat_map(|v| v.iter())
            .find_map(|e| e.message.clone())
            .unwrap_or_else(|| "Invalid input.".into());
        session.put("flash_error", msg.as_ref());
        return Redirect::to("/login").into_response();
    }}

    let result = crate::app::Models::User::User::find_by_email(&ctx.state.services.db, &req.email).await;
    match result {{
        Ok(Some(u)) if Hash::check(&req.password, &u.password) => {{
            Auth::login(&session, u.id as i64);
            Redirect::to("/dashboard").into_response()
        }}
        _ => {{
            session.put("flash_error", "Invalid email or password.");
            Redirect::to("/login").into_response()
        }}
    }}
}}

/// POST /logout
pub async fn destroy(session: Session) -> impl IntoResponse {{
    Auth::logout(&session);
    Redirect::to("/login")
}}
"#,
        name = name
    )
}

pub fn make_auth_register_controller(name: &str) -> String {
    format!(
        r#"use axum::{{
    extract::Form,
    response::{{IntoResponse, Redirect}},
}};

use {name}::{{AppError, Auth, Context, Hash, Session, view}};
use crate::app::Http::Requests::RegisterRequest::RegisterRequest;

/// GET /register
pub async fn show(ctx: Context, session: Session) -> Result<impl IntoResponse, AppError> {{
    let error: Option<String> = session.get("flash_error");
    session.forget("flash_error");
    Ok(view(&ctx, "auth.register", minijinja::context! {{ flash_error => error }})?)
}}

/// POST /register
pub async fn store(
    ctx: Context,
    session: Session,
    Form(req): Form<RegisterRequest>,
) -> impl IntoResponse {{
    use validator::Validate;
    if let Err(errors) = req.validate() {{
        let msg = errors.field_errors()
            .values()
            .flat_map(|v| v.iter())
            .find_map(|e| e.message.clone())
            .unwrap_or_else(|| "Invalid input.".into());
        session.put("flash_error", msg.as_ref());
        return Redirect::to("/register").into_response();
    }}

    let hashed = match Hash::make(&req.password) {{
        Ok(h) => h,
        Err(_) => {{
            session.put("flash_error", "An internal error occurred. Please try again.");
            return Redirect::to("/register").into_response();
        }}
    }};

    let result = sqlx::query_as::<_, crate::app::Models::User::User>(
        "INSERT INTO users (name, email, password) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&req.name)
    .bind(&req.email)
    .bind(&hashed)
    .fetch_one(&ctx.state.services.db)
    .await;

    match result {{
        Ok(_) => Redirect::to("/login").into_response(),
        Err(sqlx::Error::Database(ref db)) if db.constraint() == Some("users_email_key") => {{
            session.put("flash_error", "That email address is already registered.");
            Redirect::to("/register").into_response()
        }}
        Err(_) => {{
            session.put("flash_error", "Registration failed. Please try again.");
            Redirect::to("/register").into_response()
        }}
    }}
}}
"#,
        name = name
    )
}

pub fn view_auth_login() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}Login{% endblock %}

{% block content %}
<div class="auth-form">
  <h1>Login</h1>
  {% if flash_error %}
  <div class="alert alert-error">{{ flash_error }}</div>
  {% endif %}
  <form method="POST" action="/login">
    <div>
      <label for="email">Email</label>
      <input type="text" id="email" name="email" autocomplete="email">
    </div>
    <div>
      <label for="password">Password</label>
      <input type="password" id="password" name="password" autocomplete="current-password">
    </div>
    <div>
      <button type="submit">Login</button>
    </div>
  </form>
  <p>Don't have an account? <a href="/register">Register</a></p>
</div>
{% endblock %}
"#
}

pub fn view_auth_register() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}Register{% endblock %}

{% block content %}
<div class="auth-form">
  <h1>Register</h1>
  {% if flash_error %}
  <div class="alert alert-error">{{ flash_error }}</div>
  {% endif %}
  <form method="POST" action="/register">
    <div>
      <label for="name">Name</label>
      <input type="text" id="name" name="name" required autocomplete="name">
    </div>
    <div>
      <label for="email">Email</label>
      <input type="text" id="email" name="email" autocomplete="email">
    </div>
    <div>
      <label for="password">Password</label>
      <input type="password" id="password" name="password" autocomplete="new-password">
    </div>
    <div>
      <button type="submit">Register</button>
    </div>
  </form>
  <p>Already have an account? <a href="/login">Login</a></p>
</div>
{% endblock %}
"#
}

pub fn auth_route_snippet() -> &'static str {
    r#"NOTE: make:auth injects routes automatically into src/routes/web.rs.
      Controllers use Form<T> and return redirects, not JSON.
      For REST API auth, use: willow make:auth --api
"#
}

pub fn make_auth_api_login_controller(name: &str) -> String {
    format!(
        r#"use axum::{{Json, http::HeaderMap, response::IntoResponse}};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use validator::Validate;

use {name}::{{AppError, Context, Hash, Jwt, ValidatedJson}};

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {{
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}}

/// POST /api/auth/login
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {{
    let user = crate::app::Models::User::User::find_by_email(&ctx.state.services.db, &req.email)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !Hash::check(&req.password, &user.password) {{
        return Err(AppError::Unauthorized);
    }}

    let token = Jwt::encode(user.id as i64)?;

    Ok(Json(json!({{
        "token": token,
        "user": {{ "id": user.id, "name": user.name, "email": user.email }}
    }})))
}}

/// POST /api/auth/logout
pub async fn destroy(
    ctx: Context,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {{
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if let Some(token) = token {{
        if let Ok(claims) = Jwt::decode(token) {{
            let now = Utc::now().timestamp() as usize;
            let remaining = claims.exp.saturating_sub(now) as u64;
            Jwt::blacklist(&claims.jti, remaining, &ctx.state.services.redis).await?;
        }}
    }}

    Ok(Json(json!({{"message": "Logged out"}})))
}}
"#,
        name = name
    )
}

pub fn make_auth_api_register_controller(name: &str) -> String {
    format!(
        r#"use axum::{{Json, response::IntoResponse, http::StatusCode}};
use serde::Deserialize;
use serde_json::json;
use validator::Validate;

use {name}::{{AppError, Context, Hash, Jwt, ValidatedJson}};

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {{
    #[validate(length(min = 1, max = 255, message = "Name is required"))]
    pub name: String,
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}}

/// POST /api/auth/register
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {{
    let hashed = Hash::make(&req.password)?;

    let u = sqlx::query_as::<_, crate::app::Models::User::User>(
        "INSERT INTO users (name, email, password) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&req.name)
    .bind(&req.email)
    .bind(&hashed)
    .fetch_one(&ctx.state.services.db)
    .await
    .map_err(|e| match e {{
        sqlx::Error::Database(ref db) if db.constraint() == Some("users_email_key")
            => AppError::Conflict("Email already taken.".to_string()),
        other => AppError::Database(other),
    }})?;

    let token = Jwt::encode(u.id as i64)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({{
            "token": token,
            "user": {{ "id": u.id, "name": u.name, "email": u.email }}
        }})),
    ))
}}
"#,
        name = name
    )
}

pub fn auth_api_route_snippet() -> &'static str {
    r#"NOTE: make:auth --api injects routes automatically into src/routes/api.rs.
      Controllers use ValidatedJson<T> and return JSON, not redirects.
"#
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


pub fn config_cache() -> &'static str {
    r#"# Cache configuration
# These values are for documentation. The app reads env vars at runtime.
# Set REDIS_CLUSTER_NODES in your .env file to configure the cluster.

[cache]
store = "redis-cluster"

# Comma-separated list of cluster node URLs.
# The client auto-discovers the full topology from these seed nodes.
nodes = [
    "redis://127.0.0.1:7001",
    "redis://127.0.0.1:7002",
    "redis://127.0.0.1:7003",
]
"#
}

pub fn docker_compose() -> &'static str {
    r#"services:
  db:
    image: postgres:16-alpine
    container_name: postgres-db
    restart: unless-stopped
    environment:
      POSTGRES_DB: willowforge
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
    ports:
      - "127.0.0.1:5432:5432"
    volumes:
      - db_data:/var/lib/postgresql/data

  redis-node-1:
    image: redis:8-alpine
    restart: unless-stopped
    container_name: redis-node-1
    network_mode: host
    command: >
      redis-server
        --bind 127.0.0.1
        --port 7001
        --cluster-enabled yes
        --cluster-config-file /data/nodes.conf
        --cluster-node-timeout 5000
        --cluster-port 17001
        --cluster-announce-ip 127.0.0.1
        --cluster-announce-port 7001
        --cluster-announce-bus-port 17001
        --appendonly yes
        --save ""
    volumes:
      - redis-node-1-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-h", "127.0.0.1", "-p", "7001", "ping"]
      interval: 3s
      timeout: 2s
      retries: 20

  redis-node-2:
    image: redis:8-alpine
    restart: unless-stopped
    container_name: redis-node-2
    network_mode: host
    command: >
      redis-server
        --bind 127.0.0.1
        --port 7002
        --cluster-enabled yes
        --cluster-config-file /data/nodes.conf
        --cluster-node-timeout 5000
        --cluster-port 17002
        --cluster-announce-ip 127.0.0.1
        --cluster-announce-port 7002
        --cluster-announce-bus-port 17002
        --appendonly yes
        --save ""
    volumes:
      - redis-node-2-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-h", "127.0.0.1", "-p", "7002", "ping"]
      interval: 3s
      timeout: 2s
      retries: 20

  redis-node-3:
    image: redis:8-alpine
    restart: unless-stopped
    container_name: redis-node-3
    network_mode: host
    command: >
      redis-server
        --bind 127.0.0.1
        --port 7003
        --cluster-enabled yes
        --cluster-config-file /data/nodes.conf
        --cluster-node-timeout 5000
        --cluster-port 17003
        --cluster-announce-ip 127.0.0.1
        --cluster-announce-port 7003
        --cluster-announce-bus-port 17003
        --appendonly yes
        --save ""
    volumes:
      - redis-node-3-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-h", "127.0.0.1", "-p", "7003", "ping"]
      interval: 3s
      timeout: 2s
      retries: 20

  redis-node-4:
    image: redis:8-alpine
    restart: unless-stopped
    container_name: redis-node-4
    network_mode: host
    command: >
      redis-server
        --bind 127.0.0.1
        --port 7004
        --cluster-enabled yes
        --cluster-config-file /data/nodes.conf
        --cluster-node-timeout 5000
        --cluster-port 17004
        --cluster-announce-ip 127.0.0.1
        --cluster-announce-port 7004
        --cluster-announce-bus-port 17004
        --appendonly yes
        --save ""
    volumes:
      - redis-node-4-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-h", "127.0.0.1", "-p", "7004", "ping"]
      interval: 3s
      timeout: 2s
      retries: 20

  redis-node-5:
    image: redis:8-alpine
    restart: unless-stopped
    container_name: redis-node-5
    network_mode: host
    command: >
      redis-server
        --bind 127.0.0.1
        --port 7005
        --cluster-enabled yes
        --cluster-config-file /data/nodes.conf
        --cluster-node-timeout 5000
        --cluster-port 17005
        --cluster-announce-ip 127.0.0.1
        --cluster-announce-port 7005
        --cluster-announce-bus-port 17005
        --appendonly yes
        --save ""
    volumes:
      - redis-node-5-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-h", "127.0.0.1", "-p", "7005", "ping"]
      interval: 3s
      timeout: 2s
      retries: 20

  redis-node-6:
    image: redis:8-alpine
    restart: unless-stopped
    container_name: redis-node-6
    network_mode: host
    command: >
      redis-server
        --bind 127.0.0.1
        --port 7006
        --cluster-enabled yes
        --cluster-config-file /data/nodes.conf
        --cluster-node-timeout 5000
        --cluster-port 17006
        --cluster-announce-ip 127.0.0.1
        --cluster-announce-port 7006
        --cluster-announce-bus-port 17006
        --appendonly yes
        --save ""
    volumes:
      - redis-node-6-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-h", "127.0.0.1", "-p", "7006", "ping"]
      interval: 3s
      timeout: 2s
      retries: 20

  redis-cluster-init:
    image: redis:8-alpine
    depends_on:
      redis-node-1: { condition: service_healthy }
      redis-node-2: { condition: service_healthy }
      redis-node-3: { condition: service_healthy }
      redis-node-4: { condition: service_healthy }
      redis-node-5: { condition: service_healthy }
      redis-node-6: { condition: service_healthy }
    network_mode: host
    command: >
      sh -c '
        redis-cli --cluster create \
          127.0.0.1:7001 \
          127.0.0.1:7002 \
          127.0.0.1:7003 \
          127.0.0.1:7004 \
          127.0.0.1:7005 \
          127.0.0.1:7006 \
          --cluster-replicas 1 \
          --cluster-yes
      '
    restart: "no"

volumes:
  db_data:
  redis-node-1-data:
  redis-node-2-data:
  redis-node-3-data:
  redis-node-4-data:
  redis-node-5-data:
  redis-node-6-data:
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
        assert!(out.contains("use my_app::{bootstrap, AppError}"));
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
        assert!(out.contains("use my_app::{AppError, Cache, Context, ValidatedJson}"));
        assert!(out.contains("Result<impl IntoResponse, AppError>"));
    }
}
