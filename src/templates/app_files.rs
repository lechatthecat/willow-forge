pub fn cargo_toml(name: &str) -> String {
    format!(
        r#"[workspace]

[package]
name = "{}"
version = "0.1.0"
edition = "2024"

[dependencies]
willow-forge-runtime = {{ git = "https://github.com/lechatthecat/willow-forge.git" }}
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
path = "src/lib.rs"

[[bin]]
name = "{}"
path = "src/main.rs"
"#,
        name, name
    )
}

pub fn env_file(jwt_secret: &str) -> String {
    format!(
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

JWT_SECRET="{jwt_secret}"
JWT_EXPIRY=3600

MAIL_MAILER=log
MAIL_HOST=127.0.0.1
MAIL_PORT=2525
MAIL_USERNAME=
MAIL_PASSWORD=
MAIL_ENCRYPTION=none
MAIL_FROM_ADDRESS=hello@example.com
MAIL_FROM_NAME="Willow Forge"
"#,
        jwt_secret = jwt_secret
    )
}

pub fn main_rs(name: &str) -> String {
    format!(
        r#"mod app;
mod middleware;
mod routes;

use anyhow::Result;
use std::sync::Arc;
use ::{name}::{{bootstrap, AppError}};
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
        app::exceptions::handler::render,
    ))
    .with_state(app_state);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Willow Forge server started on http://{{}}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}}
"#,
        name = name
    )
}

pub fn bootstrap_lib_rs() -> &'static str {
    r#"pub use willow_forge_runtime::{
    AppError, AppState, Auth, AuthConfig, AuthUser, Cache, CacheConfig, Config, Context,
    DatabaseConfig, Email, Hash, Jwt, JwtConfig, JwtUser, MailConfig, Mailer, RedisCluster,
    RedisConfig, Services, Session, SessionConfig, Throttle, ValidatedJson, ViewEngine,
    authenticate, email_verification_hash, random_token, session_middleware, sign,
    verify_signature, view,
};

mod app_service_provider;

use anyhow::Result;
use minijinja::Environment;
use std::sync::Arc;

pub async fn bootstrap() -> Result<Arc<AppState>> {
    let config = Config::load()?;

    let views = build_view_engine()?;

    let db     = app_service_provider::create_pool(&config.database)?;
    let redis  = app_service_provider::create_redis_cluster(&config.redis.nodes)?;
    let mailer = Mailer::from_config(&config.mail)?;

    let services = Services { db, redis, mailer };

    Ok(Arc::new(AppState {
        config,
        services,
        views,
    }))
}

fn build_view_engine() -> Result<ViewEngine> {
    let mut env = Environment::new();
    let views_dir = std::path::PathBuf::from("src/resources/views");
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

use ::{name}::AppState;

/// Returns true if the client expects a JSON response.
///
/// Checks:
/// - Path starts with `/api/` (API routes always return JSON)
/// - OR Accept header contains `application/json`, `/json`, or `+json`
/// - OR Content-Type: application/json (client is sending JSON ↁEexpects JSON back)
/// - OR the request is an AJAX call (X-Requested-With: XMLHttpRequest) with Accept: */* or absent
fn expects_json(request: &Request) -> bool {{
    // API routes always return JSON regardless of Accept header
    if request.uri().path().starts_with("/api/") {{
        return true;
    }}

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

    // ajax()  EX-Requested-With: XMLHttpRequest
    let is_ajax = request
        .headers()
        .get("x-requested-with")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("xmlhttprequest"))
        .unwrap_or(false);

    // acceptsAnyContentType()  EAccept: */* or header absent
    let accepts_any = accept.is_empty() || accept.contains("*/*");

    wants_json || sends_json || (is_ajax && accepts_any)
}}

/// Exception handler  Eintercepts error responses and renders HTML error views when available.
///
/// How it works:
/// - Runs on every response (as the outermost layer in main.rs)
/// - If `expects_json()` is true, passes through as-is
/// - Otherwise, if the status is 4xx/5xx, looks for src/resources/views/errors/{{code}}.jinja.html
/// - If found, replaces the response with the rendered HTML view
/// - If not found, passes through the original response unchanged
///
/// To add a custom error view, create src/resources/views/errors/404.jinja.html etc.
/// To add shared logic (logging, alerting), add it inside this function.
/// To force JSON for specific paths, modify expects_json().
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

{% block title %}404  ENot Found | {{ app_name }}{% endblock %}

{% block content %}
<h1>{{ code }}</h1>
<p>{{ message }}</p>
<p><a href="/">ↁEBack to home</a></p>
{% endblock %}
"#
}

pub fn view_error_500_html() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}500  EServer Error | {{ app_name }}{% endblock %}

{% block content %}
<h1>{{ code }}</h1>
<p>{{ message }}</p>
{% endblock %}
"#
}

pub fn view_error_generic_html() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}{{ code }}  E{{ message }} | {{ app_name }}{% endblock %}

{% block content %}
<h1>{{ code }}</h1>
<p>{{ message }}</p>
{% endblock %}
"#
}

pub fn app_service_provider() -> &'static str {
    r#"use anyhow::{Context, Result};
use redis::cluster::ClusterClient;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::sync::Arc;

use crate::DatabaseConfig;

pub fn create_pool(config: &DatabaseConfig) -> Result<PgPool> {
    let opts = PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .database(&config.database)
        .username(&config.username)
        .password(&config.password)
        .ssl_mode(config.pg_ssl_mode()?);

    Ok(PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect_lazy_with(opts))
}

/// Build a Redis cluster client from a list of node URLs.
///
/// Only validates config syntax  Eno actual connection is made here.
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

use ::{name}::AppState;
use crate::app::http::controllers::{{user_controller, status_controller}};

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
        r#"use axum::{{routing::get, Router}};
use std::sync::Arc;

use ::{name}::AppState;
use crate::app::http::controllers::home_controller;

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

use ::{name}::{{AppError, Context}};
use ::{name}::view::view;

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

use ::{name}::{{AppError, Cache, Context, Hash, ValidatedJson}};
use crate::app::models::user::User;

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
//   sqlx::Error     ↁEAppError::Database    (via #[from])
//   ViewError       ↁEAppError::View        (via #[from])
//   ValidationError ↁEAppError::Validation  (via #[from])
//   redis::RedisError ↁEAppError::Redis     (via #[from])
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
    let hashed = Hash::make(&req.password)?;

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (name, email, password)
         VALUES ($1, $2, $3)
         RETURNING id, name, email, password, created_at",
    )
    .bind(&req.name)
    .bind(&req.email)
    .bind(&hashed)
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

use ::{name}::Context;

pub async fn index(ctx: Context) -> impl IntoResponse {{
    // Both checks run concurrently  Eneither blocks the other.
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
        .link-button { background: none; border: none; padding: 0; color: inherit; font: inherit; cursor: pointer; text-decoration: underline; }
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

{% block title %}Welcome  E{{ app_name }}{% endblock %}

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
<pre><code>docker compose -f src/docker/docker-compose.yml up -d --build</code></pre>
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
        <tr><td><code>POST</code></td><td><form method="POST" action="/logout" style="display:inline;margin:0"><button type="submit" class="link-button"><code>/logout</code></button></form></td><td>Logout</td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/register">/register</a></code></td><td>Registration form</td></tr>
        <tr><td><code>POST</code></td><td><code>/register</code></td><td>Submit registration</td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/dashboard">/dashboard</a></code></td><td>Protected dashboard (requires login)</td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/forgot-password">/forgot-password</a></code></td><td>Forgot password form</td></tr>
        <tr><td><code>POST</code></td><td><code>/forgot-password</code></td><td>Email a reset link</td></tr>
        <tr><td><code>GET</code></td><td><code>/reset-password/{token}</code></td><td>Reset password form</td></tr>
        <tr><td><code>POST</code></td><td><code>/reset-password</code></td><td>Submit new password</td></tr>
        <tr><td><code>GET</code></td><td><code><a href="/email/verify">/email/verify</a></code></td><td>Email verification notice</td></tr>
        <tr><td><code>GET</code></td><td><code>/email/verify/{id}/{hash}</code></td><td>Confirm email address</td></tr>
        <tr><td><code>POST</code></td><td><code>/email/verification-notification</code></td><td>Resend verification email</td></tr>
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

<h3>POST /api/users  Esuccess</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d '{"name":"Alice","email":"alice@example.com","password":"secret123"}' | jq</code></pre>

<h3>POST /api/users  Evalidation error (422)</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d '{"name":"","email":"not-an-email","password":"short"}' | jq</code></pre>

<h3>POST /api/users  Emalformed JSON (400)</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d 'invalid' | jq</code></pre>

<h2>JWT API Auth (after <code>willow-forge make:auth --api</code>)</h2>
<p>Scaffold JWT-based auth (register, login, password reset, email verification, refresh, logout; JSON only, no views):</p>
<pre><code>willow-forge make:auth --api</code></pre>
<p>Routes injected into <code>src/routes/api.rs</code>. Restart the server to activate them.</p>
<table>
    <thead>
        <tr><th>Method</th><th>URL</th><th>Description</th></tr>
    </thead>
    <tbody>
        <tr><td><code>POST</code></td><td><code>/api/auth/register</code></td><td>Create account, returns JWT</td></tr>
        <tr><td><code>POST</code></td><td><code>/api/auth/login</code></td><td>Authenticate, returns JWT</td></tr>
        <tr><td><code>POST</code></td><td><code>/api/auth/forgot-password</code></td><td>Email a password reset token</td></tr>
        <tr><td><code>POST</code></td><td><code>/api/auth/reset-password</code></td><td>Reset password with emailed token</td></tr>
        <tr><td><code>GET</code></td><td><code>/api/auth/email/verify/{id}/{hash}</code></td><td>Confirm email address</td></tr>
        <tr><td><code>POST</code></td><td><code>/api/auth/email/verification-notification</code></td><td>Resend verification email (requires JWT)</td></tr>
        <tr><td><code>POST</code></td><td><code>/api/auth/refresh</code></td><td>Rotate JWT (old token blacklisted)</td></tr>
        <tr><td><code>POST</code></td><td><code>/api/auth/logout</code></td><td>Blacklist token, invalidate session</td></tr>
        <tr><td><code>GET</code></td><td><code>/api/me</code></td><td>Current user info (requires JWT)</td></tr>
    </tbody>
</table>

<h3>Register</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"name":"Alice","email":"alice@example.com","password":"secret123"}' | jq</code></pre>

<h3>Login</h3>
<pre><code>TOKEN=$(curl -s -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"alice@example.com","password":"secret123"}' | jq -r '.token')
echo $TOKEN</code></pre>

<h3>Forgot password</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/auth/forgot-password \
  -H "Content-Type: application/json" \
  -d '{"email":"alice@example.com"}' | jq</code></pre>

<h3>Reset password</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/auth/reset-password \
  -H "Content-Type: application/json" \
  -d '{"email":"alice@example.com","token":"TOKEN_FROM_EMAIL","password":"newsecret123","password_confirmation":"newsecret123"}' | jq</code></pre>

<h3>Resend verification email</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/auth/email/verification-notification \
  -H "Authorization: Bearer $TOKEN" | jq</code></pre>

<h3>Refresh</h3>
<pre><code>NEW_TOKEN=$(curl -s -X POST http://localhost:3000/api/auth/refresh \
  -H "Authorization: Bearer $TOKEN" | jq -r '.token')
echo $NEW_TOKEN</code></pre>

<h3>Logout</h3>
<pre><code>curl -s -X POST http://localhost:3000/api/auth/logout \
  -H "Authorization: Bearer $TOKEN" | jq</code></pre>

<h2>Docker Hints</h2>
<h3>Get inside a container</h3>
<pre><code>docker exec -it redis-node-1 sh</code></pre>

<h3>Logs</h3>
<pre><code>docker logs --tail 50 --follow --timestamps postgres-db
docker logs --tail 50 --follow --timestamps redis-node-1</code></pre>

<h3>List containers</h3>
<pre><code>docker ps -a</code></pre>

<h3>Stop containers</h3>
<pre><code>docker compose -f src/docker/docker-compose.yml down</code></pre>

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
docker compose -f src/docker/docker-compose.yml restart redis-cluster-init</code></pre>
<p>PowerShell:</p>
<pre><code>foreach ($port in 7001, 7002, 7003, 7004, 7005, 7006) { $node = $port - 7000; docker exec redis-node-$node redis-cli -p $port FLUSHALL; docker exec redis-node-$node redis-cli -p $port CLUSTER RESET }; docker compose -f src/docker/docker-compose.yml restart redis-cluster-init</code></pre>


<script>
    fetch('/api/status')
        .then(r => r.json())
        .then(data => {
            // DB status
            const dbOk = data.db;
            document.getElementById('db-status').textContent = dbOk ? 'Connected' : 'Not connected';
            const dbCls = dbOk ? 'badge-db-ok' : 'badge-db-off';
            const dbLabel = dbOk ? 'DB connected' : 'DB not connected';
            ['db-badge-1', 'db-badge-2'].forEach(id => {
                const el = document.getElementById(id);
                el.className = 'badge ' + dbCls;
                el.textContent = dbLabel;
            });

            // Redis status
            const redisOk = data.redis;
            const redisEl = document.getElementById('redis-status');
            redisEl.textContent = redisOk ? 'Connected' : 'Not connected';
            redisEl.style.color = redisOk ? '#0a3622' : '#58151c';
        })
        .catch(() => {
            document.getElementById('db-status').textContent = 'Not connected';
            ['db-badge-1', 'db-badge-2'].forEach(id => {
                const el = document.getElementById(id);
                el.className = 'badge badge-db-off';
                el.textContent = 'DB not connected';
            });
            const redisEl = document.getElementById('redis-status');
            redisEl.textContent = 'Not connected';
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
password = "postgres"
max_connections = 10
ssl_mode = "disable"
"#
}

pub fn src_app_mod_rs() -> &'static str {
    "pub mod http;\npub mod models;\npub mod exceptions;\n"
}

pub fn src_app_http_mod_rs() -> &'static str {
    "pub mod controllers;\npub mod middleware;\npub mod requests;\n"
}

pub fn src_app_http_controllers_mod_rs() -> &'static str {
    "pub mod home_controller;\npub mod user_controller;\npub mod status_controller;\n"
}

pub fn src_app_http_middleware_mod_rs() -> &'static str {
    "pub mod log_request;\n"
}

pub fn src_app_http_requests_mod_rs() -> &'static str {
    ""
}

pub fn src_app_models_mod_rs() -> &'static str {
    "pub mod user;\n"
}

pub fn models_mod_rs() -> &'static str {
    src_app_models_mod_rs()
}

pub fn src_app_exceptions_mod_rs() -> &'static str {
    "pub mod handler;\n"
}

pub fn src_routes_mod_rs() -> &'static str {
    "pub mod web;\npub mod api;\n"
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
    #[serde(skip_serializing, default)]
    pub password: String,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub async fn find_by_email(db: &PgPool, email: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT id, name, email, password, created_at FROM users WHERE email = $1 LIMIT 1",
        )
            .bind(email)
            .fetch_optional(db)
            .await
    }

    pub async fn find(db: &PgPool, id: i32) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT id, name, email, password, created_at FROM users WHERE id = $1 LIMIT 1",
        )
            .bind(id)
            .fetch_optional(db)
            .await
    }
}
"#
}

pub fn initial_migration_up_sql() -> &'static str {
    r#"CREATE TABLE IF NOT EXISTS users (
    id                SERIAL PRIMARY KEY,
    name              VARCHAR(255)  NOT NULL,
    email             VARCHAR(255)  NOT NULL UNIQUE,
    password          VARCHAR(255)  NOT NULL,
    created_at        TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);
"#
}

pub fn initial_migration_down_sql() -> &'static str {
    "DROP TABLE IF EXISTS users;\n"
}

pub fn email_verification_migration_up_sql() -> &'static str {
    r#"ALTER TABLE users
    ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMPTZ;
"#
}

pub fn email_verification_migration_down_sql() -> &'static str {
    r#"ALTER TABLE users
    DROP COLUMN IF EXISTS email_verified_at;
"#
}

pub fn password_reset_migration_up_sql() -> &'static str {
    r#"CREATE TABLE IF NOT EXISTS password_reset_tokens (
    email      VARCHAR(255)  PRIMARY KEY,
    token      VARCHAR(255)  NOT NULL,
    created_at TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);
"#
}

pub fn password_reset_migration_down_sql() -> &'static str {
    "DROP TABLE IF EXISTS password_reset_tokens;\n"
}

pub fn bootstrap_middleware_rs(name: &str) -> String {
    format!(
        r#"use crate::app::http::middleware::log_request;

use axum::{{extract::Request, middleware, middleware::Next, Router}};
use std::sync::Arc;

use ::{name}::{{AppState, session_middleware}};

/// Global middleware  Eruns on every request.
pub fn global(state: Arc<AppState>, router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    let sess = Arc::clone(&state);
    router
        .layer(middleware::from_fn(log_request::handle))
        .layer(middleware::from_fn(move |req: Request, next: Next| {{
            let s = Arc::clone(&sess);
            async move {{ session_middleware(s, req, next).await }}
        }}))
}}

/// Web middleware  Eruns only on HTML routes (src/routes/web.rs).
pub fn web(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {{
    router
    // Protect all web routes with auth:
    // .layer(middleware::from_fn(::{name}::authenticate))
}}

/// API middleware  Eruns only on API routes (src/routes/api.rs).
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
        "{} {} ↁE{} ({:?})",
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
///   use crate::app::http::middleware::{name};
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
session_enabled = false
session_lifetime = 7200
session_cookie = "willow_session"
session_secure = false
"#
}

pub fn config_jwt() -> &'static str {
    r#"[jwt]
secret = ""
expiry = 3600
"#
}

pub fn config_mail() -> &'static str {
    r#"[mail]
mailer = "log"
host = "127.0.0.1"
port = 2525
username = ""
password = ""
encryption = "none"
from_address = "hello@example.com"
from_name = "Willow Forge"
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

use ::{name}::{{AppError, Auth, Context, Hash, Session, view}};
use crate::app::http::requests::login_request::LoginRequest;

/// GET /login
pub async fn show(ctx: Context, session: Session) -> Result<impl IntoResponse, AppError> {{
    let error: Option<String> = session.get("flash_error");
    let old_email: Option<String> = session.get("flash_old_email");
    session.forget("flash_error");
    session.forget("flash_old_email");
    Ok(view(&ctx, "auth.login", minijinja::context! {{
        flash_error => error,
        old_email => old_email.unwrap_or_default(),
    }})?)
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
        session.put("flash_old_email", req.email.as_str());
        return Redirect::to("/login").into_response();
    }}

    let result = crate::app::models::user::User::find_by_email(&ctx.state.services.db, &req.email).await;
    match result {{
        Ok(Some(u)) if Hash::check(&req.password, &u.password) => {{
            Auth::login(&session, u.id as i64);
            Redirect::to("/dashboard").into_response()
        }}
        _ => {{
            session.put("flash_error", "Invalid email or password.");
            session.put("flash_old_email", req.email.as_str());
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

use ::{name}::{{AppError, Context, Hash, Session, view}};
use crate::app::http::requests::register_request::RegisterRequest;

/// GET /register
pub async fn show(ctx: Context, session: Session) -> Result<impl IntoResponse, AppError> {{
    let error: Option<String> = session.get("flash_error");
    let old_name: Option<String> = session.get("flash_old_name");
    let old_email: Option<String> = session.get("flash_old_email");
    session.forget("flash_error");
    session.forget("flash_old_name");
    session.forget("flash_old_email");
    Ok(view(&ctx, "auth.register", minijinja::context! {{
        flash_error => error,
        old_name => old_name.unwrap_or_default(),
        old_email => old_email.unwrap_or_default(),
    }})?)
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
        session.put("flash_old_name", req.name.as_str());
        session.put("flash_old_email", req.email.as_str());
        return Redirect::to("/register").into_response();
    }}

    let hashed = match Hash::make(&req.password) {{
        Ok(h) => h,
        Err(_) => {{
            session.put("flash_error", "An internal error occurred. Please try again.");
            session.put("flash_old_name", req.name.as_str());
            session.put("flash_old_email", req.email.as_str());
            return Redirect::to("/register").into_response();
        }}
    }};

    let result = sqlx::query_as::<_, crate::app::models::user::User>(
        "INSERT INTO users (name, email, password) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&req.name)
    .bind(&req.email)
    .bind(&hashed)
    .fetch_one(&ctx.state.services.db)
    .await;

    match result {{
        Ok(user) => {{
            // Send the email-verification link; best-effort, never blocks signup.
            crate::app::http::controllers::auth::verify_email_controller::send_verification_email(&ctx, &user).await;
            Redirect::to("/login").into_response()
        }}
        Err(sqlx::Error::Database(ref db)) if db.constraint() == Some("users_email_key") => {{
            session.put("flash_error", "That email address is already registered.");
            session.put("flash_old_name", req.name.as_str());
            session.put("flash_old_email", req.email.as_str());
            Redirect::to("/register").into_response()
        }}
        Err(_) => {{
            session.put("flash_error", "Registration failed. Please try again.");
            session.put("flash_old_name", req.name.as_str());
            session.put("flash_old_email", req.email.as_str());
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
      <input type="text" id="email" name="email" autocomplete="email" value="{{ old_email }}">
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
      <input type="text" id="name" name="name" required autocomplete="name" value="{{ old_name }}">
    </div>
    <div>
      <label for="email">Email</label>
      <input type="text" id="email" name="email" autocomplete="email" value="{{ old_email }}">
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

pub fn make_auth_dashboard_controller(name: &str) -> String {
    format!(
        r#"use axum::response::IntoResponse;
use ::{name}::{{AppError, Context, view}};
use crate::app::http::middleware::ensure_email_verified::VerifiedUser;

/// GET /dashboard  Erequires session login
pub async fn index(user: VerifiedUser, ctx: Context) -> Result<impl IntoResponse, AppError> {{
    Ok(view(&ctx, "dashboard", minijinja::context! {{ user_id => user.id }})?)
}}
"#,
        name = name
    )
}

pub fn view_auth_dashboard() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}Dashboard{% endblock %}

{% block content %}
<h1>Dashboard</h1>
<p>Logged in as user ID: <strong>{{ user_id }}</strong></p>
<form method="POST" action="/logout" style="margin-top:1rem">
    <button type="submit">Logout</button>
</form>
{% endblock %}
"#
}

pub fn auth_route_use_decl() -> &'static str {
    "use crate::app::http::controllers::auth::{login_controller, register_controller, forgot_password_controller, reset_password_controller, verify_email_controller};\nuse crate::app::http::controllers::dashboard_controller;"
}

pub fn auth_route_snippet() -> &'static str {
    "\n        .route(\"/login\",    get(login_controller::show).post(login_controller::store))\n        .route(\"/logout\",   post(login_controller::destroy))\n        .route(\"/register\", get(register_controller::show).post(register_controller::store))\n        .route(\"/dashboard\", get(dashboard_controller::index))\n        .route(\"/forgot-password\", get(forgot_password_controller::show).post(forgot_password_controller::store))\n        .route(\"/reset-password/{token}\", get(reset_password_controller::show))\n        .route(\"/reset-password\", post(reset_password_controller::store))\n        .route(\"/email/verify\", get(verify_email_controller::notice))\n        .route(\"/email/verify/{id}/{hash}\", get(verify_email_controller::verify))\n        .route(\"/email/verification-notification\", post(verify_email_controller::resend))"
}

// ── Password reset scaffolding templates ───────────────────────────────────────

pub fn make_auth_forgot_password_request() -> &'static str {
    r#"use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,
}
"#
}

pub fn make_auth_reset_password_request() -> &'static str {
    r#"use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    #[validate(email(message = "Must be a valid email address"))]
    pub email: String,

    #[validate(length(min = 1, message = "Reset token is required"))]
    pub token: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,

    #[validate(must_match(other = "password", message = "Password confirmation does not match"))]
    pub password_confirmation: String,
}
"#
}

pub fn make_auth_forgot_password_controller(name: &str) -> String {
    format!(
        r#"use axum::{{
    extract::Form,
    response::{{IntoResponse, Redirect}},
}};

use ::{name}::{{AppError, Context, Email, Hash, Session, Throttle, random_token, view}};
use crate::app::http::requests::forgot_password_request::ForgotPasswordRequest;

/// How long a password reset link stays valid, in minutes.
const TOKEN_TTL_MINUTES: i64 = 60;

/// Max reset requests allowed per email within the throttle window.
const MAX_ATTEMPTS: u64 = 5;
/// Throttle window, in seconds.
const THROTTLE_WINDOW_SECS: i64 = 60;

/// GET /forgot-password
pub async fn show(ctx: Context, session: Session) -> Result<impl IntoResponse, AppError> {{
    let error: Option<String> = session.get("flash_error");
    let status: Option<String> = session.get("flash_status");
    session.forget("flash_error");
    session.forget("flash_status");
    Ok(view(&ctx, "auth.forgot-password", minijinja::context! {{
        flash_error => error,
        flash_status => status,
    }})?)
}}

/// POST /forgot-password
pub async fn store(
    ctx: Context,
    session: Session,
    Form(req): Form<ForgotPasswordRequest>,
) -> impl IntoResponse {{
    use validator::Validate;
    if let Err(errors) = req.validate() {{
        let msg = errors.field_errors()
            .values()
            .flat_map(|v| v.iter())
            .find_map(|e| e.message.clone())
            .unwrap_or_else(|| "Invalid input.".into());
        session.put("flash_error", msg.as_ref());
        return Redirect::to("/forgot-password").into_response();
    }}

    // Same response whether or not the email exists, to avoid leaking which
    // addresses are registered.
    let neutral = "If that email address exists in our records, a password reset link has been sent.";

    // Throttle per email so the endpoint can't be used to bomb an inbox. When
    // the limit is hit, keep the neutral response and skip sending.
    let throttle_key = format!("throttle:forgot-password:{{}}", req.email);
    if Throttle::too_many(&ctx, &throttle_key, MAX_ATTEMPTS, THROTTLE_WINDOW_SECS)
        .await
        .unwrap_or(false)
    {{
        session.put("flash_status", neutral);
        return Redirect::to("/forgot-password").into_response();
    }}

    if let Ok(Some(_user)) =
        crate::app::models::user::User::find_by_email(&ctx.state.services.db, &req.email).await
    {{
        let token = random_token();
        if let Ok(hashed_token) = Hash::make(&token) {{
            let stored = sqlx::query(
                "INSERT INTO password_reset_tokens (email, token) VALUES ($1, $2) \
                 ON CONFLICT (email) DO UPDATE SET token = EXCLUDED.token, created_at = NOW()",
            )
            .bind(&req.email)
            .bind(&hashed_token)
            .execute(&ctx.state.services.db)
            .await;

            if stored.is_ok() {{
                let base = ctx.state.config.app_url.clone();
                let link = format!("{{}}/reset-password/{{}}", base.trim_end_matches('/'), token);
                let text = format!(
                    "You requested a password reset.\n\nOpen this link to choose a new password \
                     (valid for {{}} minutes):\n{{}}\n\nIf you did not request this, ignore this email.",
                    TOKEN_TTL_MINUTES, link,
                );
                let html = format!(
                    "<p>You requested a password reset.</p>\
                     <p><a href=\"{{}}\">Choose a new password</a> (valid for {{}} minutes).</p>\
                     <p>If you did not request this, you can ignore this email.</p>",
                    link, TOKEN_TTL_MINUTES,
                );
                let email = Email::new(req.email.as_str(), "Reset your password")
                    .text(text)
                    .html(html);
                if let Err(e) = ctx.state.services.mailer.send(&email).await {{
                    tracing::error!("Failed to send password reset email: {{}}", e);
                }}
            }}
        }}
    }}

    session.put("flash_status", neutral);
    Redirect::to("/forgot-password").into_response()
}}
"#,
        name = name
    )
}

pub fn make_auth_reset_password_controller(name: &str) -> String {
    format!(
        r#"use axum::{{
    extract::{{Form, Path}},
    response::{{IntoResponse, Redirect}},
}};
use chrono::{{DateTime, Duration, Utc}};
use sqlx::FromRow;

use ::{name}::{{AppError, Context, Hash, Session, view}};
use crate::app::http::requests::reset_password_request::ResetPasswordRequest;

/// How long a password reset link stays valid, in minutes.
const TOKEN_TTL_MINUTES: i64 = 60;

#[derive(FromRow)]
struct ResetRow {{
    token: String,
    created_at: DateTime<Utc>,
}}

/// GET /reset-password/{{token}}
pub async fn show(
    ctx: Context,
    session: Session,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, AppError> {{
    let error: Option<String> = session.get("flash_error");
    let old_email: Option<String> = session.get("flash_old_email");
    session.forget("flash_error");
    session.forget("flash_old_email");
    Ok(view(&ctx, "auth.reset-password", minijinja::context! {{
        token => token,
        flash_error => error,
        old_email => old_email.unwrap_or_default(),
    }})?)
}}

/// POST /reset-password
pub async fn store(
    ctx: Context,
    session: Session,
    Form(req): Form<ResetPasswordRequest>,
) -> impl IntoResponse {{
    use validator::Validate;
    let back = format!("/reset-password/{{}}", req.token);

    if let Err(errors) = req.validate() {{
        let msg = errors.field_errors()
            .values()
            .flat_map(|v| v.iter())
            .find_map(|e| e.message.clone())
            .unwrap_or_else(|| "Invalid input.".into());
        session.put("flash_error", msg.as_ref());
        session.put("flash_old_email", req.email.as_str());
        return Redirect::to(&back).into_response();
    }}

    let row = sqlx::query_as::<_, ResetRow>(
        "SELECT token, created_at FROM password_reset_tokens WHERE email = $1 LIMIT 1",
    )
    .bind(&req.email)
    .fetch_optional(&ctx.state.services.db)
    .await;

    let invalid = "This password reset link is invalid or has expired.";
    let row = match row {{
        Ok(Some(r)) => r,
        _ => {{
            session.put("flash_error", invalid);
            session.put("flash_old_email", req.email.as_str());
            return Redirect::to(&back).into_response();
        }}
    }};

    let expired = Utc::now() - row.created_at > Duration::minutes(TOKEN_TTL_MINUTES);
    if expired || !Hash::check(&req.token, &row.token) {{
        session.put("flash_error", invalid);
        session.put("flash_old_email", req.email.as_str());
        return Redirect::to(&back).into_response();
    }}

    let hashed = match Hash::make(&req.password) {{
        Ok(h) => h,
        Err(_) => {{
            session.put("flash_error", "An internal error occurred. Please try again.");
            return Redirect::to(&back).into_response();
        }}
    }};

    let updated = sqlx::query("UPDATE users SET password = $1 WHERE email = $2")
        .bind(&hashed)
        .bind(&req.email)
        .execute(&ctx.state.services.db)
        .await;

    if updated.is_err() {{
        session.put("flash_error", "Could not reset your password. Please try again.");
        return Redirect::to(&back).into_response();
    }}

    let _ = sqlx::query("DELETE FROM password_reset_tokens WHERE email = $1")
        .bind(&req.email)
        .execute(&ctx.state.services.db)
        .await;

    session.put("flash_status", "Your password has been reset. You can now log in.");
    Redirect::to("/login").into_response()
}}
"#,
        name = name
    )
}

pub fn view_auth_forgot_password() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}Forgot Password{% endblock %}

{% block content %}
<div class="auth-form">
  <h1>Forgot Password</h1>
  {% if flash_status %}
  <div class="alert alert-success">{{ flash_status }}</div>
  {% endif %}
  {% if flash_error %}
  <div class="alert alert-error">{{ flash_error }}</div>
  {% endif %}
  <p>Enter your email address and we'll send you a password reset link.</p>
  <form method="POST" action="/forgot-password">
    <div>
      <label for="email">Email</label>
      <input type="text" id="email" name="email" autocomplete="email">
    </div>
    <div>
      <button type="submit">Send reset link</button>
    </div>
  </form>
  <p><a href="/login">Back to login</a></p>
</div>
{% endblock %}
"#
}

pub fn view_auth_reset_password() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}Reset Password{% endblock %}

{% block content %}
<div class="auth-form">
  <h1>Reset Password</h1>
  {% if flash_error %}
  <div class="alert alert-error">{{ flash_error }}</div>
  {% endif %}
  <form method="POST" action="/reset-password">
    <input type="hidden" name="token" value="{{ token }}">
    <div>
      <label for="email">Email</label>
      <input type="text" id="email" name="email" autocomplete="email" value="{{ old_email }}">
    </div>
    <div>
      <label for="password">New Password</label>
      <input type="password" id="password" name="password" autocomplete="new-password">
    </div>
    <div>
      <label for="password_confirmation">Confirm Password</label>
      <input type="password" id="password_confirmation" name="password_confirmation" autocomplete="new-password">
    </div>
    <div>
      <button type="submit">Reset password</button>
    </div>
  </form>
</div>
{% endblock %}
"#
}

// ── Email verification scaffolding templates ───────────────────────────────────

pub fn make_ensure_email_verified_middleware(name: &str) -> String {
    format!(
        r#"use std::sync::Arc;

use axum::{{
    extract::FromRequestParts,
    http::{{header, request::Parts, StatusCode}},
    response::{{IntoResponse, Redirect, Response}},
}};

use ::{name}::{{AppState, AuthUser, Context}};

/// Guard extractor for routes that require a verified email address.
///
/// Resolves the logged-in user (like `AuthUser`), then checks the
/// `email_verified_at` column. Unverified users are redirected to
/// `/email/verify` (web) or rejected with 403 (API / JSON clients).
///
/// Add `_verified: VerifiedUser` to any handler to enforce verification.
pub struct VerifiedUser {{
    pub id: i64,
}}

impl<S> FromRequestParts<S> for VerifiedUser
where
    S: Send + Sync,
    Arc<AppState>: axum::extract::FromRef<S>,
{{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {{
        // Must be authenticated first; reuse AuthUser's own rejection.
        let auth = AuthUser::from_request_parts(parts, state).await?;

        let ctx = Context::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;

        let verified_at = sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT email_verified_at FROM users WHERE id = $1",
        )
        .bind(auth.id as i32)
        .fetch_optional(&ctx.state.services.db)
        .await;

        match verified_at {{
            Ok(Some(Some(_))) => Ok(VerifiedUser {{ id: auth.id }}),
            Ok(_) => Err(unverified_response(parts)),
            Err(e) => {{
                tracing::error!("Verified guard query failed: {{}}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR.into_response())
            }}
        }}
    }}
}}

fn unverified_response(parts: &Parts) -> Response {{
    let wants_json = parts.uri.path().starts_with("/api/")
        || parts
            .headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(|a| a.contains("application/json"))
            .unwrap_or(false);

    if wants_json {{
        (StatusCode::FORBIDDEN, "Your email address is not verified.").into_response()
    }} else {{
        Redirect::to("/email/verify").into_response()
    }}
}}
"#,
        name = name
    )
}

pub fn make_auth_verify_email_controller(name: &str) -> String {
    format!(
        r#"use axum::{{
    extract::{{Path, Query}},
    response::{{IntoResponse, Redirect, Response}},
}};
use chrono::{{DateTime, Duration, Utc}};
use serde::Deserialize;

use ::{name}::{{
    AppError, AuthUser, Context, Email, Jwt, Session, Throttle, email_verification_hash, sign,
    verify_signature, view,
}};
use crate::app::models::user::User;

/// How long a verification link stays valid, in minutes.
const VERIFY_TTL_MINUTES: i64 = 60;
/// Max resend requests allowed per user within the throttle window.
const MAX_RESEND_ATTEMPTS: u64 = 3;
/// Throttle window, in seconds.
const RESEND_WINDOW_SECS: i64 = 60;

async fn user_is_verified(ctx: &Context, user_id: i32) -> Result<bool, AppError> {{
    let verified_at = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
        "SELECT email_verified_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&ctx.state.services.db)
    .await?;

    Ok(verified_at.flatten().is_some())
}}

/// Query parameters appended to a verification link: `?expires=..&signature=..`.
#[derive(Deserialize)]
pub struct VerifyQuery {{
    pub expires: i64,
    pub signature: String,
}}

/// GET /email/verify  Eprompt the user to verify their address.
pub async fn notice(
    _auth: AuthUser,
    ctx: Context,
    session: Session,
) -> Result<impl IntoResponse, AppError> {{
    let status: Option<String> = session.get("flash_status");
    session.forget("flash_status");
    Ok(view(&ctx, "auth.verify-email", minijinja::context! {{
        flash_status => status,
    }})?)
}}

/// GET /email/verify/{{id}}/{{hash}}  Econfirm the address from the emailed link.
pub async fn verify(
    ctx: Context,
    Path((id, hash)): Path<(i32, String)>,
    Query(query): Query<VerifyQuery>,
) -> Response {{
    let forbidden =
        || (axum::http::StatusCode::FORBIDDEN, "Invalid or expired verification link.").into_response();

    // Reject expired links.
    if Utc::now().timestamp() > query.expires {{
        return forbidden();
    }}

    // Reject tampered links: signature must match `id.hash.expires`.
    let payload = format!("{{}}.{{}}.{{}}", id, hash, query.expires);
    if Jwt::validate_secret(&ctx.state.config.jwt).is_err() {{
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }}
    if !verify_signature(&payload, &query.signature, &ctx.state.config.jwt.secret) {{
        return forbidden();
    }}

    let user = match User::find(&ctx.state.services.db, id).await {{
        Ok(Some(u)) => u,
        _ => return forbidden(),
    }};

    if email_verification_hash(&user.email) != hash {{
        return forbidden();
    }}

    if let Err(e) = sqlx::query(
        "UPDATE users SET email_verified_at = NOW() WHERE id = $1 AND email_verified_at IS NULL",
    )
    .bind(id)
    .execute(&ctx.state.services.db)
    .await
    {{
        tracing::error!("Failed to mark email as verified: {{}}", e);
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }}

    Redirect::to("/dashboard").into_response()
}}

/// POST /email/verification-notification  Eresend the verification link.
pub async fn resend(
    auth: AuthUser,
    ctx: Context,
    session: Session,
) -> impl IntoResponse {{
    // Throttle per user so resend can't be used to bomb an inbox.
    let throttle_key = format!("throttle:verify-resend:{{}}", auth.id);
    if Throttle::too_many(&ctx, &throttle_key, MAX_RESEND_ATTEMPTS, RESEND_WINDOW_SECS)
        .await
        .unwrap_or(false)
    {{
        session.put("flash_status", "Please wait a moment before requesting another verification email.");
        return Redirect::to("/email/verify");
    }}

    if let Ok(Some(user)) = User::find(&ctx.state.services.db, auth.id as i32).await {{
        if !user_is_verified(&ctx, user.id).await.unwrap_or_else(|e| {{
            tracing::error!("Failed to read email verification status: {{}}", e);
            true
        }}) {{
            send_verification_email(&ctx, &user).await;
        }}
    }}
    session.put("flash_status", "A fresh verification link has been sent to your email address.");
    Redirect::to("/email/verify")
}}

/// Build and send the verification email for `user`. Best-effort: failures are
/// logged but do not surface to the caller.
pub async fn send_verification_email(ctx: &Context, user: &User) {{
    if let Err(e) = Jwt::validate_secret(&ctx.state.config.jwt) {{
        tracing::error!("Cannot send verification email with insecure JWT secret: {{}}", e);
        return;
    }}

    let base = ctx.state.config.app_url.clone();
    let hash = email_verification_hash(&user.email);
    let expires = (Utc::now() + Duration::minutes(VERIFY_TTL_MINUTES)).timestamp();
    let payload = format!("{{}}.{{}}.{{}}", user.id, hash, expires);
    let signature = sign(&payload, &ctx.state.config.jwt.secret);
    let link = format!(
        "{{}}/email/verify/{{}}/{{}}?expires={{}}&signature={{}}",
        base.trim_end_matches('/'), user.id, hash, expires, signature,
    );

    let text = format!(
        "Welcome! Please confirm your email address by opening this link:\n{{}}\n",
        link,
    );
    let html = format!(
        "<p>Welcome! Please confirm your email address.</p>\
         <p><a href=\"{{}}\">Verify email address</a></p>",
        link,
    );
    let email = Email::new(user.email.as_str(), "Verify your email address")
        .text(text)
        .html(html);
    if let Err(e) = ctx.state.services.mailer.send(&email).await {{
        tracing::error!("Failed to send verification email: {{}}", e);
    }}
}}
"#,
        name = name
    )
}

pub fn make_auth_api_forgot_password_controller(name: &str) -> String {
    format!(
        r#"use axum::{{Json, response::IntoResponse}};
use serde_json::json;

use ::{name}::{{AppError, Context, Email, Hash, Throttle, ValidatedJson, random_token}};
use crate::app::http::requests::forgot_password_request::ForgotPasswordRequest;

/// How long a password reset token stays valid, in minutes.
const TOKEN_TTL_MINUTES: i64 = 60;

/// Max reset requests allowed per email within the throttle window.
const MAX_ATTEMPTS: u64 = 5;
/// Throttle window, in seconds.
const THROTTLE_WINDOW_SECS: i64 = 60;

/// POST /api/auth/forgot-password
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<ForgotPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {{
    // Same response whether or not the email exists, to avoid leaking which
    // addresses are registered.
    let neutral = "If that email address exists in our records, a password reset link has been sent.";

    // Throttle per email so the endpoint can't be used to bomb an inbox. When
    // the limit is hit, keep the neutral response and skip sending.
    let throttle_key = format!("throttle:forgot-password:{{}}", req.email);
    if Throttle::too_many(&ctx, &throttle_key, MAX_ATTEMPTS, THROTTLE_WINDOW_SECS)
        .await
        .unwrap_or(false)
    {{
        return Ok(Json(json!({{ "message": neutral }})));
    }}

    if let Ok(Some(_user)) =
        crate::app::models::user::User::find_by_email(&ctx.state.services.db, &req.email).await
    {{
        let token = random_token();
        if let Ok(hashed_token) = Hash::make(&token) {{
            let stored = sqlx::query(
                "INSERT INTO password_reset_tokens (email, token) VALUES ($1, $2) \
                 ON CONFLICT (email) DO UPDATE SET token = EXCLUDED.token, created_at = NOW()",
            )
            .bind(&req.email)
            .bind(&hashed_token)
            .execute(&ctx.state.services.db)
            .await;

            if stored.is_ok() {{
                let base = ctx.state.config.app_url.clone();
                let link = format!(
                    "{{}}/api/auth/reset-password?token={{}}",
                    base.trim_end_matches('/'),
                    token,
                );
                let text = format!(
                    "You requested a password reset.\n\nUse this token to reset your password \
                     (valid for {{}} minutes):\n{{}}\n\nEndpoint: POST {{}}\n\nIf you did not request this, ignore this email.",
                    TOKEN_TTL_MINUTES, token, link,
                );
                let html = format!(
                    "<p>You requested a password reset.</p>\
                     <p>Use this token to reset your password (valid for {{}} minutes):</p>\
                     <p><code>{{}}</code></p>\
                     <p>Endpoint: <code>POST {{}}</code></p>\
                     <p>If you did not request this, you can ignore this email.</p>",
                    TOKEN_TTL_MINUTES, token, link,
                );
                let email = Email::new(req.email.as_str(), "Reset your password")
                    .text(text)
                    .html(html);
                if let Err(e) = ctx.state.services.mailer.send(&email).await {{
                    tracing::error!("Failed to send password reset email: {{}}", e);
                }}
            }}
        }}
    }}

    Ok(Json(json!({{ "message": neutral }})))
}}
"#,
        name = name
    )
}

pub fn make_auth_api_reset_password_controller(name: &str) -> String {
    format!(
        r#"use axum::{{Json, response::IntoResponse}};
use chrono::{{DateTime, Duration, Utc}};
use serde_json::json;
use sqlx::FromRow;

use ::{name}::{{AppError, Context, Hash, ValidatedJson}};
use crate::app::http::requests::reset_password_request::ResetPasswordRequest;

/// How long a password reset token stays valid, in minutes.
const TOKEN_TTL_MINUTES: i64 = 60;

#[derive(FromRow)]
struct ResetRow {{
    token: String,
    created_at: DateTime<Utc>,
}}

/// POST /api/auth/reset-password
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<ResetPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {{
    let invalid = "This password reset token is invalid or has expired.";

    let row = sqlx::query_as::<_, ResetRow>(
        "SELECT token, created_at FROM password_reset_tokens WHERE email = $1 LIMIT 1",
    )
    .bind(&req.email)
    .fetch_optional(&ctx.state.services.db)
    .await?;

    let Some(row) = row else {{
        return Err(AppError::Http(422, invalid.to_string()));
    }};

    let expired = Utc::now() - row.created_at > Duration::minutes(TOKEN_TTL_MINUTES);
    if expired || !Hash::check(&req.token, &row.token) {{
        return Err(AppError::Http(422, invalid.to_string()));
    }}

    let hashed = Hash::make(&req.password)?;
    sqlx::query("UPDATE users SET password = $1 WHERE email = $2")
        .bind(&hashed)
        .bind(&req.email)
        .execute(&ctx.state.services.db)
        .await?;

    sqlx::query("DELETE FROM password_reset_tokens WHERE email = $1")
        .bind(&req.email)
        .execute(&ctx.state.services.db)
        .await?;

    Ok(Json(json!({{ "message": "Your password has been reset." }})))
}}
"#,
        name = name
    )
}

pub fn make_auth_api_verify_email_controller(name: &str) -> String {
    format!(
        r#"use axum::{{
    extract::{{Path, Query}},
    http::StatusCode,
    response::{{IntoResponse, Response}},
    Json,
}};
use chrono::{{DateTime, Duration, Utc}};
use serde::Deserialize;
use serde_json::json;

use ::{name}::{{
    AppError, Context, Email, Jwt, JwtUser, Throttle, email_verification_hash, sign,
    verify_signature,
}};
use crate::app::models::user::User;

/// How long a verification link stays valid, in minutes.
const VERIFY_TTL_MINUTES: i64 = 60;
/// Max resend requests allowed per user within the throttle window.
const MAX_RESEND_ATTEMPTS: u64 = 3;
/// Throttle window, in seconds.
const RESEND_WINDOW_SECS: i64 = 60;

async fn user_is_verified(ctx: &Context, user_id: i32) -> Result<bool, AppError> {{
    let verified_at = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
        "SELECT email_verified_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&ctx.state.services.db)
    .await?;

    Ok(verified_at.flatten().is_some())
}}

/// Query parameters appended to a verification link: `?expires=..&signature=..`.
#[derive(Deserialize)]
pub struct VerifyQuery {{
    pub expires: i64,
    pub signature: String,
}}

/// GET /api/auth/email/verify/{{id}}/{{hash}}
pub async fn verify(
    ctx: Context,
    Path((id, hash)): Path<(i32, String)>,
    Query(query): Query<VerifyQuery>,
) -> Response {{
    let forbidden = || {{
        (
            StatusCode::FORBIDDEN,
            Json(json!({{ "message": "Invalid or expired verification link." }})),
        )
            .into_response()
    }};

    // Reject expired links.
    if Utc::now().timestamp() > query.expires {{
        return forbidden();
    }}

    // Reject tampered links: signature must match `id.hash.expires`.
    let payload = format!("{{}}.{{}}.{{}}", id, hash, query.expires);
    if Jwt::validate_secret(&ctx.state.config.jwt).is_err() {{
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({{ "message": "Internal server error" }})),
        )
            .into_response();
    }}
    if !verify_signature(&payload, &query.signature, &ctx.state.config.jwt.secret) {{
        return forbidden();
    }}

    let user = match User::find(&ctx.state.services.db, id).await {{
        Ok(Some(u)) => u,
        _ => return forbidden(),
    }};

    if email_verification_hash(&user.email) != hash {{
        return forbidden();
    }}

    if let Err(e) = sqlx::query(
        "UPDATE users SET email_verified_at = NOW() WHERE id = $1 AND email_verified_at IS NULL",
    )
    .bind(id)
    .execute(&ctx.state.services.db)
    .await
    {{
        tracing::error!("Failed to mark email as verified: {{}}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({{ "message": "Internal server error" }})),
        )
            .into_response();
    }}

    Json(json!({{ "message": "Email verified." }})).into_response()
}}

/// POST /api/auth/email/verification-notification
pub async fn resend(
    auth: JwtUser,
    ctx: Context,
) -> Result<Response, AppError> {{
    // Throttle per user so resend can't be used to bomb an inbox.
    let throttle_key = format!("throttle:verify-resend:{{}}", auth.id);
    if Throttle::too_many(&ctx, &throttle_key, MAX_RESEND_ATTEMPTS, RESEND_WINDOW_SECS)
        .await
        .unwrap_or(false)
    {{
        return Ok((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({{ "message": "Please wait a moment before requesting another verification email." }})),
        )
            .into_response());
    }}

    if let Some(user) = User::find(&ctx.state.services.db, auth.id as i32).await? {{
        if !user_is_verified(&ctx, user.id).await? {{
            send_verification_email(&ctx, &user).await;
            return Ok(Json(json!({{
                "message": "A fresh verification link has been sent to your email address."
            }}))
            .into_response());
        }}
    }}

    Ok(Json(json!({{ "message": "Your email address is already verified." }})).into_response())
}}

/// Build and send the verification email for `user`. Best-effort: failures are
/// logged but do not surface to the caller.
pub async fn send_verification_email(ctx: &Context, user: &User) {{
    if let Err(e) = Jwt::validate_secret(&ctx.state.config.jwt) {{
        tracing::error!("Cannot send verification email with insecure JWT secret: {{}}", e);
        return;
    }}

    let base = ctx.state.config.app_url.clone();
    let hash = email_verification_hash(&user.email);
    let expires = (Utc::now() + Duration::minutes(VERIFY_TTL_MINUTES)).timestamp();
    let payload = format!("{{}}.{{}}.{{}}", user.id, hash, expires);
    let signature = sign(&payload, &ctx.state.config.jwt.secret);
    let link = format!(
        "{{}}/api/auth/email/verify/{{}}/{{}}?expires={{}}&signature={{}}",
        base.trim_end_matches('/'), user.id, hash, expires, signature,
    );

    let text = format!(
        "Welcome! Please confirm your email address by opening this link:\n{{}}\n",
        link,
    );
    let html = format!(
        "<p>Welcome! Please confirm your email address.</p>\
         <p><a href=\"{{}}\">Verify email address</a></p>",
        link,
    );
    let email = Email::new(user.email.as_str(), "Verify your email address")
        .text(text)
        .html(html);
    if let Err(e) = ctx.state.services.mailer.send(&email).await {{
        tracing::error!("Failed to send verification email: {{}}", e);
    }}
}}
"#,
        name = name
    )
}

pub fn view_auth_verify_email() -> &'static str {
    r#"{% extends "layouts.app" %}

{% block title %}Verify Email{% endblock %}

{% block content %}
<div class="auth-form">
  <h1>Verify Your Email</h1>
  {% if flash_status %}
  <div class="alert alert-success">{{ flash_status }}</div>
  {% endif %}
  <p>Thanks for signing up! Before getting started, please verify your email
     address by clicking the link we just emailed to you. If you didn't receive
     it, request another below.</p>
  <form method="POST" action="/email/verification-notification">
    <button type="submit">Resend verification email</button>
  </form>
  <form method="POST" action="/logout" style="margin-top:1rem">
    <button type="submit">Logout</button>
  </form>
</div>
{% endblock %}
"#
}

pub fn make_auth_api_login_controller(name: &str) -> String {
    format!(
        r#"use axum::{{Json, http::HeaderMap, response::IntoResponse}};
use chrono::Utc;
use serde_json::json;

use ::{name}::{{AppError, Context, Hash, Jwt, JwtUser, ValidatedJson}};
use crate::app::http::requests::login_request::LoginRequest;

/// POST /api/auth/login
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {{
    let user = crate::app::models::user::User::find_by_email(&ctx.state.services.db, &req.email)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !Hash::check(&req.password, &user.password) {{
        return Err(AppError::Unauthorized);
    }}

    let token = Jwt::encode_with_config(user.id as i64, &ctx.state.config.jwt)?;

    Ok(Json(json!({{
        "token": token,
        "user": {{ "id": user.id, "name": user.name, "email": user.email }}
    }})))
}}

/// POST /api/auth/refresh
pub async fn refresh(
    ctx: Context,
    auth: JwtUser,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {{
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if let Some(token) = token {{
        if let Ok(claims) = Jwt::decode_with_config(token, &ctx.state.config.jwt) {{
            let now = Utc::now().timestamp() as usize;
            let remaining = claims.exp.saturating_sub(now) as u64;
            Jwt::blacklist(&claims.jti, remaining, &ctx.state.services.redis).await?;
        }}
    }}

    let new_token = Jwt::encode_with_config(auth.id, &ctx.state.config.jwt)?;
    Ok(Json(json!({{"token": new_token}})))
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
        if let Ok(claims) = Jwt::decode_with_config(token, &ctx.state.config.jwt) {{
            let now = Utc::now().timestamp() as usize;
            let remaining = claims.exp.saturating_sub(now) as u64;
            Jwt::blacklist(&claims.jti, remaining, &ctx.state.services.redis).await?;
        }}
    }}

    Ok(Json(json!({{"message": "Logged out"}})))
}}

/// GET /api/me  Ereturns the authenticated user's ID
pub async fn me(auth: JwtUser) -> impl IntoResponse {{
    Json(json!({{"user_id": auth.id}}))
}}
"#,
        name = name
    )
}

pub fn make_auth_api_register_controller(name: &str) -> String {
    format!(
        r#"use axum::{{Json, response::IntoResponse, http::StatusCode}};
use serde_json::json;

use ::{name}::{{AppError, Context, Hash, Jwt, ValidatedJson}};
use crate::app::http::requests::register_request::RegisterRequest;

/// POST /api/auth/register
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {{
    let hashed = Hash::make(&req.password)?;

    let u = sqlx::query_as::<_, crate::app::models::user::User>(
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

    // Send the email-verification link; best-effort, never blocks signup.
    crate::app::http::controllers::auth::api_verify_email_controller::send_verification_email(&ctx, &u).await;

    let token = Jwt::encode_with_config(u.id as i64, &ctx.state.config.jwt)?;

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

pub fn auth_api_route_use_decl() -> &'static str {
    "use crate::app::http::controllers::auth::{api_forgot_password_controller, api_login_controller, api_register_controller, api_reset_password_controller, api_verify_email_controller};"
}

pub fn auth_api_route_snippet() -> &'static str {
    "\n        .route(\"/api/auth/login\", post(api_login_controller::store))\n        .route(\"/api/auth/refresh\", post(api_login_controller::refresh))\n        .route(\"/api/auth/logout\", post(api_login_controller::destroy))\n        .route(\"/api/auth/register\", post(api_register_controller::store))\n        .route(\"/api/auth/forgot-password\", post(api_forgot_password_controller::store))\n        .route(\"/api/auth/reset-password\", post(api_reset_password_controller::store))\n        .route(\"/api/auth/email/verify/{id}/{hash}\", get(api_verify_email_controller::verify))\n        .route(\"/api/auth/email/verification-notification\", post(api_verify_email_controller::resend))\n        .route(\"/api/me\", get(api_login_controller::me))"
}

fn pascal_to_snake(name: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = name.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            let prev = i.checked_sub(1).and_then(|idx| chars.get(idx)).copied();
            let next = chars.get(i + 1).copied();
            let boundary_after_lower = prev
                .map(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                .unwrap_or(false);
            let boundary_before_word = prev
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && next.map(|c| c.is_ascii_lowercase()).unwrap_or(false);

            if (boundary_after_lower || boundary_before_word) && !result.ends_with('_') && !result.is_empty() {
                result.push('_');
            }
            for lower in ch.to_lowercase() {
                result.push(lower);
            }
        } else if ch == '-' || ch == ' ' || ch == '/' || ch == '\\' {
            if !result.ends_with('_') && !result.is_empty() {
                result.push('_');
            }
        } else {
            result.push(ch);
        }
    }

    result.trim_matches('_').to_string()
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
    r#"[cache]
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
    fn cargo_toml_uses_git_runtime_dependency() {
        let out = cargo_toml("my-app");
        assert!(out.contains(
            "willow-forge-runtime = { git = \"https://github.com/lechatthecat/willow-forge.git\" }"
        ));
        let path_key = ["pa", "th"].concat();
        assert!(
            !out.lines()
                .any(|line| line.trim_start().starts_with("willow-forge-runtime")
                    && line.contains(&path_key))
        );
    }

    #[test]
    fn env_file_uses_generated_jwt_secret() {
        let out = env_file("generated-secret-for-tests-1234567890");
        assert!(out.contains("JWT_SECRET=\"generated-secret-for-tests-1234567890\""));
        assert!(!out.contains("JWT_SECRET=change-me-in-production"));
    }

    #[test]
    fn auth_config_disables_sessions_by_default() {
        let out = config_auth();
        assert!(out.contains("session_enabled = false"));
    }

    #[test]
    fn base_users_migration_has_no_auth_verification_column() {
        assert!(!initial_migration_up_sql().contains("email_verified_at"));
    }

    #[test]
    fn email_verification_migration_adds_auth_column() {
        assert!(email_verification_migration_up_sql().contains("email_verified_at"));
        assert!(email_verification_migration_up_sql().contains("ADD COLUMN IF NOT EXISTS"));
        assert!(email_verification_migration_down_sql().contains("DROP COLUMN IF EXISTS"));
    }

    #[test]
    fn main_rs_uses_crate_name_in_import() {
        let out = main_rs("my_app");
        assert!(out.contains("use ::my_app::{bootstrap, AppError}"));
    }

    #[test]
    fn welcome_view_status_script_has_valid_status_strings() {
        let out = view_welcome();
        assert!(out.contains("dbOk ? 'Connected' : 'Not connected'"));
        assert!(out.contains("redisOk ? 'Connected' : 'Not connected'"));
        assert!(out.contains("textContent = 'Not connected';"));
        assert!(!out.contains(&format!("Connected {}", "\u{2701}E")));
        assert!(!out.contains(&format!("Not connected {}", "\u{2701}E")));
    }

    #[test]
    fn routes_api_uses_crate_name() {
        let out = routes_api("my_app");
        assert!(out.contains("use ::my_app::AppState"));
    }

    #[test]
    fn routes_web_uses_crate_name() {
        let out = routes_web("my_app");
        assert!(out.contains("use ::my_app::AppState"));
    }

    #[test]
    fn home_controller_uses_crate_name() {
        let out = home_controller("my_app");
        assert!(out.contains("use ::my_app::{AppError, Context}"));
        assert!(out.contains("Result<impl IntoResponse, AppError>"));
    }

    #[test]
    fn user_controller_uses_crate_name() {
        let out = user_controller("my_app");
        assert!(out.contains("use ::my_app::{AppError, Cache, Context, Hash, ValidatedJson}"));
        assert!(out.contains("Result<impl IntoResponse, AppError>"));
    }

    // --- auth template regression tests ---

    // Bug: web LoginController used wrong module path (LoginRequest::LoginRequest)
    #[test]
    fn web_login_controller_imports_snake_case_module() {
        let out = make_auth_login_controller("my_app");
        assert!(out.contains("login_request::LoginRequest"),
            "must import from login_request:: (snake_case module name)");
        assert!(!out.contains("LoginRequest::LoginRequest"),
            "must not use PascalCase module path");
    }

    // Bug: web RegisterController used wrong module path (RegisterRequest::RegisterRequest)
    #[test]
    fn web_register_controller_imports_snake_case_module() {
        let out = make_auth_register_controller("my_app");
        assert!(out.contains("register_request::RegisterRequest"),
            "must import from register_request:: (snake_case module name)");
        assert!(!out.contains("RegisterRequest::RegisterRequest"),
            "must not use PascalCase module path");
    }

    // Bug: web LoginController was missing `show` (API version was generated instead)
    #[test]
    fn web_login_controller_has_show_function() {
        let out = make_auth_login_controller("my_app");
        assert!(out.contains("pub async fn show("),
            "web LoginController must have show() for GET /login");
    }

    // Bug: web RegisterController was missing `show`
    #[test]
    fn web_register_controller_has_show_function() {
        let out = make_auth_register_controller("my_app");
        assert!(out.contains("pub async fn show("),
            "web RegisterController must have show() for GET /register");
    }

    // Bug: API login controller defined LoginRequest inline instead of importing it
    #[test]
    fn api_login_controller_imports_login_request_not_inline() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.contains("login_request::LoginRequest"),
            "must import LoginRequest from login_request module");
        assert!(!out.contains("struct LoginRequest"),
            "must not define LoginRequest struct inline");
    }

    // Bug: API register controller defined RegisterRequest inline instead of importing it
    #[test]
    fn api_register_controller_imports_register_request_not_inline() {
        let out = make_auth_api_register_controller("my_app");
        assert!(out.contains("register_request::RegisterRequest"),
            "must import RegisterRequest from register_request module");
        assert!(!out.contains("struct RegisterRequest"),
            "must not define RegisterRequest struct inline");
    }

    // Bug: API login controller was missing refresh endpoint
    #[test]
    fn api_login_controller_has_refresh_function() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.contains("pub async fn refresh("),
            "ApiLoginController must have refresh() for POST /api/auth/refresh");
        assert!(out.contains("JwtUser"),
            "refresh() must accept JwtUser extractor");
    }

    // Bug: API login controller was missing logout endpoint
    #[test]
    fn api_login_controller_has_destroy_function() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.contains("pub async fn destroy("),
            "ApiLoginController must have destroy() for POST /api/auth/logout");
    }

    // Invariant: no template may use #[path]  Eit is permanently banned
    #[test]
    fn no_template_uses_path_attribute() {
        let cases: &[(&str, &str)] = &[
            ("make_auth_login_controller", &make_auth_login_controller("my_app")),
            ("make_auth_register_controller", &make_auth_register_controller("my_app")),
            ("make_auth_api_login_controller", &make_auth_api_login_controller("my_app")),
            ("make_auth_api_register_controller", &make_auth_api_register_controller("my_app")),
            ("make_auth_api_forgot_password_controller", &make_auth_api_forgot_password_controller("my_app")),
            ("make_auth_api_reset_password_controller", &make_auth_api_reset_password_controller("my_app")),
            ("make_auth_api_verify_email_controller", &make_auth_api_verify_email_controller("my_app")),
        ];
        for (label, out) in cases {
            assert!(!out.contains("#[path"),
                "{label}: must not contain #[path] attribute");
        }
    }

    // Invariant: login request has both email and password fields
    #[test]
    fn login_request_has_required_fields() {
        let out = make_auth_login_request();
        assert!(out.contains("pub email: String"));
        assert!(out.contains("pub password: String"));
        assert!(out.contains("#[derive") && out.contains("Validate"));
    }

    // Invariant: register request has name, email, password
    #[test]
    fn register_request_has_required_fields() {
        let out = make_auth_register_request();
        assert!(out.contains("pub name: String"));
        assert!(out.contains("pub email: String"));
        assert!(out.contains("pub password: String"));
        assert!(out.contains("#[derive") && out.contains("Validate"));
    }

    // ── API login controller (50) ──────────────────────────────────────────────

    // api_01. has store function
    #[test]
    fn api_01_has_store_fn() {
        assert!(make_auth_api_login_controller("my_app").contains("pub async fn store("));
    }
    // api_02. has refresh function
    #[test]
    fn api_02_has_refresh_fn() {
        assert!(make_auth_api_login_controller("my_app").contains("pub async fn refresh("));
    }
    // api_03. has destroy function
    #[test]
    fn api_03_has_destroy_fn() {
        assert!(make_auth_api_login_controller("my_app").contains("pub async fn destroy("));
    }
    // api_04. has me function
    #[test]
    fn api_04_has_me_fn() {
        assert!(make_auth_api_login_controller("my_app").contains("pub async fn me("));
    }
    // api_05. imports LoginRequest from snake_case module
    #[test]
    fn api_05_imports_login_request_snake_case() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.contains("login_request::LoginRequest"));
        assert!(!out.contains("LoginRequest::LoginRequest"));
    }
    // api_06. imports AppError
    #[test]
    fn api_06_imports_app_error() {
        assert!(make_auth_api_login_controller("my_app").contains("AppError"));
    }
    // api_07. imports Context
    #[test]
    fn api_07_imports_context() {
        assert!(make_auth_api_login_controller("my_app").contains("Context"));
    }
    // api_08. imports Hash
    #[test]
    fn api_08_imports_hash() {
        assert!(make_auth_api_login_controller("my_app").contains("Hash"));
    }
    // api_09. imports Jwt
    #[test]
    fn api_09_imports_jwt() {
        assert!(make_auth_api_login_controller("my_app").contains("Jwt"));
    }
    // api_10. imports JwtUser
    #[test]
    fn api_10_imports_jwt_user() {
        assert!(make_auth_api_login_controller("my_app").contains("JwtUser"));
    }
    // api_11. imports ValidatedJson
    #[test]
    fn api_11_imports_validated_json() {
        assert!(make_auth_api_login_controller("my_app").contains("ValidatedJson"));
    }
    // api_12. no #[path] attribute
    #[test]
    fn api_12_no_path_attribute() {
        assert!(!make_auth_api_login_controller("my_app").contains("#[path"));
    }
    // api_13. no inline struct LoginRequest
    #[test]
    fn api_13_no_inline_login_request_struct() {
        assert!(!make_auth_api_login_controller("my_app").contains("struct LoginRequest"));
    }
    // api_14. store accepts ValidatedJson<LoginRequest>
    #[test]
    fn api_14_store_accepts_validated_json() {
        assert!(make_auth_api_login_controller("my_app").contains("ValidatedJson(req): ValidatedJson<LoginRequest>"));
    }
    // api_15. store returns Result<impl IntoResponse, AppError>
    #[test]
    fn api_15_store_returns_result() {
        assert!(make_auth_api_login_controller("my_app").contains("Result<impl IntoResponse, AppError>"));
    }
    // api_16. store uses find_by_email
    #[test]
    fn api_16_store_uses_find_by_email() {
        assert!(make_auth_api_login_controller("my_app").contains("find_by_email"));
    }
    // api_17. store uses Hash::check
    #[test]
    fn api_17_store_uses_hash_check() {
        assert!(make_auth_api_login_controller("my_app").contains("Hash::check"));
    }
    // api_18. store uses Jwt::encode_with_config
    #[test]
    fn api_18_store_uses_jwt_encode() {
        assert!(make_auth_api_login_controller("my_app").contains("Jwt::encode_with_config"));
    }
    // api_19. store returns 401 on wrong password
    #[test]
    fn api_19_store_returns_unauthorized_on_wrong_password() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.contains("AppError::Unauthorized"));
    }
    // api_20. store returns 401 when user not found
    #[test]
    fn api_20_store_returns_unauthorized_user_not_found() {
        assert!(make_auth_api_login_controller("my_app").contains("ok_or(AppError::Unauthorized)"));
    }
    // api_21. store response includes token field
    #[test]
    fn api_21_store_response_has_token() {
        assert!(make_auth_api_login_controller("my_app").contains("\"token\""));
    }
    // api_22. store response includes user object
    #[test]
    fn api_22_store_response_has_user() {
        assert!(make_auth_api_login_controller("my_app").contains("\"user\""));
    }
    // api_23. store response includes user.id
    #[test]
    fn api_23_store_response_has_user_id() {
        assert!(make_auth_api_login_controller("my_app").contains("user.id"));
    }
    // api_24. store response includes user.name
    #[test]
    fn api_24_store_response_has_user_name() {
        assert!(make_auth_api_login_controller("my_app").contains("user.name"));
    }
    // api_25. store response includes user.email
    #[test]
    fn api_25_store_response_has_user_email() {
        assert!(make_auth_api_login_controller("my_app").contains("user.email"));
    }
    // api_26. refresh accepts JwtUser extractor
    #[test]
    fn api_26_refresh_accepts_jwt_user() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.contains("auth: JwtUser"));
    }
    // api_27. refresh accepts HeaderMap
    #[test]
    fn api_27_refresh_accepts_header_map() {
        assert!(make_auth_api_login_controller("my_app").contains("headers: HeaderMap"));
    }
    // api_28. refresh decodes existing token
    #[test]
    fn api_28_refresh_uses_jwt_decode() {
        assert!(make_auth_api_login_controller("my_app").contains("Jwt::decode_with_config"));
    }
    // api_29. refresh blacklists old token
    #[test]
    fn api_29_refresh_uses_jwt_blacklist() {
        assert!(make_auth_api_login_controller("my_app").contains("Jwt::blacklist"));
    }
    // api_30. refresh issues new token via Jwt::encode_with_config
    #[test]
    fn api_30_refresh_encodes_new_token() {
        let out = make_auth_api_login_controller("my_app");
        let encode_count = out.matches("Jwt::encode_with_config").count();
        assert!(encode_count >= 2, "Jwt::encode_with_config must appear in both store and refresh");
    }
    // api_31. refresh response includes token field
    #[test]
    fn api_31_refresh_response_has_token() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.contains("new_token") || out.contains("\"token\""));
    }
    // api_32. refresh strips Bearer prefix
    #[test]
    fn api_32_refresh_strips_bearer_prefix() {
        assert!(make_auth_api_login_controller("my_app").contains("Bearer "));
    }
    // api_33. refresh uses Utc::now for expiry calculation
    #[test]
    fn api_33_refresh_uses_utc_now() {
        assert!(make_auth_api_login_controller("my_app").contains("Utc::now()"));
    }
    // api_34. refresh reads claims.exp
    #[test]
    fn api_34_refresh_reads_claims_exp() {
        assert!(make_auth_api_login_controller("my_app").contains("claims.exp"));
    }
    // api_35. destroy accepts HeaderMap
    #[test]
    fn api_35_destroy_accepts_header_map() {
        let out = make_auth_api_login_controller("my_app");
        let destroy_pos = out.find("pub async fn destroy(").unwrap();
        assert!(out[destroy_pos..].contains("HeaderMap"));
    }
    // api_36. destroy decodes token
    #[test]
    fn api_36_destroy_uses_jwt_decode() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.matches("Jwt::decode_with_config").count() >= 2, "both refresh and destroy must decode");
    }
    // api_37. destroy blacklists token
    #[test]
    fn api_37_destroy_uses_jwt_blacklist() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.matches("Jwt::blacklist").count() >= 2, "both refresh and destroy must blacklist");
    }
    // api_38. destroy strips Bearer prefix
    #[test]
    fn api_38_destroy_strips_bearer_prefix() {
        let out = make_auth_api_login_controller("my_app");
        assert!(out.matches("Bearer ").count() >= 2, "both refresh and destroy strip Bearer prefix");
    }
    // api_39. destroy returns Logged out message
    #[test]
    fn api_39_destroy_returns_logged_out() {
        assert!(make_auth_api_login_controller("my_app").contains("Logged out"));
    }
    // api_40. me accepts JwtUser
    #[test]
    fn api_40_me_accepts_jwt_user() {
        let out = make_auth_api_login_controller("my_app");
        let me_pos = out.find("pub async fn me(").unwrap();
        assert!(out[me_pos..].contains("JwtUser"));
    }
    // api_41. me returns user_id in JSON
    #[test]
    fn api_41_me_returns_user_id() {
        assert!(make_auth_api_login_controller("my_app").contains("user_id"));
    }
    // api_42. me uses auth.id
    #[test]
    fn api_42_me_uses_auth_id() {
        assert!(make_auth_api_login_controller("my_app").contains("auth.id"));
    }
    // api_43. imports from crate::app::http::requests::login_request
    #[test]
    fn api_43_imports_from_requests_module() {
        assert!(make_auth_api_login_controller("my_app")
            .contains("crate::app::http::requests::login_request"));
    }
    // api_44. uses provided crate name in runtime import
    #[test]
    fn api_44_uses_crate_name_in_import() {
        let out = make_auth_api_login_controller("my_crate");
        assert!(out.contains("use ::my_crate::"));
    }
    // api_45. imports HeaderMap from axum::http
    #[test]
    fn api_45_imports_header_map_from_axum() {
        assert!(make_auth_api_login_controller("my_app").contains("HeaderMap"));
    }
    // api_46. imports Json from axum
    #[test]
    fn api_46_imports_json() {
        assert!(make_auth_api_login_controller("my_app").contains("Json"));
    }
    // api_47. imports IntoResponse
    #[test]
    fn api_47_imports_into_response() {
        assert!(make_auth_api_login_controller("my_app").contains("IntoResponse"));
    }
    // api_48. imports chrono Utc
    #[test]
    fn api_48_imports_chrono_utc() {
        assert!(make_auth_api_login_controller("my_app").contains("chrono::Utc"));
    }
    // api_49. uses serde_json json! macro
    #[test]
    fn api_49_uses_json_macro() {
        assert!(make_auth_api_login_controller("my_app").contains("json!"));
    }
    // api_50. no Japanese text
    #[test]
    fn api_50_no_japanese_text() {
        let out = make_auth_api_login_controller("my_app");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c)
            || ('\u{4E00}'..='\u{9FFF}').contains(&c)));
    }

    // ── web login controller (50) ──────────────────────────────────────────────

    // web_01. has show function
    #[test]
    fn web_lc_01_has_show_fn() {
        assert!(make_auth_login_controller("my_app").contains("pub async fn show("));
    }
    // web_02. has store function
    #[test]
    fn web_lc_02_has_store_fn() {
        assert!(make_auth_login_controller("my_app").contains("pub async fn store("));
    }
    // web_03. has destroy function
    #[test]
    fn web_lc_03_has_destroy_fn() {
        assert!(make_auth_login_controller("my_app").contains("pub async fn destroy("));
    }
    // web_04. imports LoginRequest from snake_case module
    #[test]
    fn web_lc_04_imports_login_request_snake_case() {
        let out = make_auth_login_controller("my_app");
        assert!(out.contains("login_request::LoginRequest"));
        assert!(!out.contains("LoginRequest::LoginRequest"));
    }
    // web_05. imports AppError
    #[test]
    fn web_lc_05_imports_app_error() {
        assert!(make_auth_login_controller("my_app").contains("AppError"));
    }
    // web_06. imports Auth
    #[test]
    fn web_lc_06_imports_auth() {
        assert!(make_auth_login_controller("my_app").contains("Auth"));
    }
    // web_07. imports Context
    #[test]
    fn web_lc_07_imports_context() {
        assert!(make_auth_login_controller("my_app").contains("Context"));
    }
    // web_08. imports Hash
    #[test]
    fn web_lc_08_imports_hash() {
        assert!(make_auth_login_controller("my_app").contains("Hash"));
    }
    // web_09. imports Session
    #[test]
    fn web_lc_09_imports_session() {
        assert!(make_auth_login_controller("my_app").contains("Session"));
    }
    // web_10. imports view
    #[test]
    fn web_lc_10_imports_view() {
        assert!(make_auth_login_controller("my_app").contains("view"));
    }
    // web_11. no #[path] attribute
    #[test]
    fn web_lc_11_no_path_attribute() {
        assert!(!make_auth_login_controller("my_app").contains("#[path"));
    }
    // web_12. no inline struct LoginRequest
    #[test]
    fn web_lc_12_no_inline_login_request_struct() {
        assert!(!make_auth_login_controller("my_app").contains("struct LoginRequest"));
    }
    // web_13. show reads flash_error from session
    #[test]
    fn web_lc_13_show_reads_flash_error() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        assert!(out[show_pos..].contains("flash_error"));
    }
    // web_14. show reads flash_old_email from session
    #[test]
    fn web_lc_14_show_reads_flash_old_email() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        assert!(out[show_pos..].contains("flash_old_email"));
    }
    // web_15. show forgets flash_error
    #[test]
    fn web_lc_15_show_forgets_flash_error() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        let show_body = &out[show_pos..store_pos];
        assert!(show_body.contains("forget(\"flash_error\")"));
    }
    // web_16. show forgets flash_old_email
    #[test]
    fn web_lc_16_show_forgets_flash_old_email() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        let show_body = &out[show_pos..store_pos];
        assert!(show_body.contains("forget(\"flash_old_email\")"));
    }
    // web_17. show passes flash_error to template context
    #[test]
    fn web_lc_17_show_passes_flash_error_to_template() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[show_pos..store_pos].contains("flash_error =>"));
    }
    // web_18. show passes old_email to template context
    #[test]
    fn web_lc_18_show_passes_old_email_to_template() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[show_pos..store_pos].contains("old_email =>"));
    }
    // web_19. show renders auth.login view
    #[test]
    fn web_lc_19_show_renders_auth_login_view() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[show_pos..store_pos].contains("auth.login"));
    }
    // web_20. show returns Result<impl IntoResponse, AppError>
    #[test]
    fn web_lc_20_show_returns_result() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[show_pos..store_pos].contains("Result<impl IntoResponse, AppError>"));
    }
    // web_21. show uses minijinja::context!
    #[test]
    fn web_lc_21_show_uses_minijinja_context() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[show_pos..store_pos].contains("minijinja::context!"));
    }
    // web_22. store accepts Form<LoginRequest>
    #[test]
    fn web_lc_22_store_accepts_form() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("Form(req): Form<LoginRequest>"));
    }
    // web_23. store validates with req.validate()
    #[test]
    fn web_lc_23_store_validates_request() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("req.validate()"));
    }
    // web_24. store puts flash_error on validation failure
    #[test]
    fn web_lc_24_store_puts_flash_error_on_validation_fail() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("put(\"flash_error\""));
    }
    // web_25. store puts flash_old_email on validation failure
    #[test]
    fn web_lc_25_store_puts_flash_old_email_on_validation_fail() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("put(\"flash_old_email\""));
    }
    // web_26. store redirects to /login on validation failure
    #[test]
    fn web_lc_26_store_redirects_to_login_on_validation_fail() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        let errs_pos = out[store_pos..].find("Err(errors)").unwrap() + store_pos;
        assert!(out[errs_pos..].contains("Redirect::to(\"/login\")"));
    }
    // web_27. store uses find_by_email
    #[test]
    fn web_lc_27_store_uses_find_by_email() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("find_by_email"));
    }
    // web_28. store uses Hash::check
    #[test]
    fn web_lc_28_store_uses_hash_check() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("Hash::check"));
    }
    // web_29. store calls Auth::login on success
    #[test]
    fn web_lc_29_store_calls_auth_login() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("Auth::login"));
    }
    // web_30. store redirects to /dashboard on success
    #[test]
    fn web_lc_30_store_redirects_to_dashboard_on_success() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("Redirect::to(\"/dashboard\")"));
    }
    // web_31. store puts flash_error on wrong password
    #[test]
    fn web_lc_31_store_puts_flash_error_on_wrong_password() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].matches("put(\"flash_error\"").count() >= 2,
            "flash_error must be set for both validation error and wrong password");
    }
    // web_32. store redirects to /login on wrong password
    #[test]
    fn web_lc_32_store_redirects_to_login_on_wrong_password() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].matches("Redirect::to(\"/login\")").count() >= 2,
            "must redirect to /login for both validation error and wrong password");
    }
    // web_33. store puts flash_old_email on wrong password
    #[test]
    fn web_lc_33_store_puts_flash_old_email_on_wrong_password() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].matches("put(\"flash_old_email\"").count() >= 2,
            "flash_old_email must be set for both validation error and wrong password");
    }
    // web_34. destroy calls Auth::logout
    #[test]
    fn web_lc_34_destroy_calls_auth_logout() {
        let out = make_auth_login_controller("my_app");
        let destroy_pos = out.find("pub async fn destroy(").unwrap();
        assert!(out[destroy_pos..].contains("Auth::logout"));
    }
    // web_35. destroy redirects to /login
    #[test]
    fn web_lc_35_destroy_redirects_to_login() {
        let out = make_auth_login_controller("my_app");
        let destroy_pos = out.find("pub async fn destroy(").unwrap();
        assert!(out[destroy_pos..].contains("Redirect::to(\"/login\")"));
    }
    // web_36. no ValidatedJson (form-based, not JSON API)
    #[test]
    fn web_lc_36_no_validated_json() {
        assert!(!make_auth_login_controller("my_app").contains("ValidatedJson"));
    }
    // web_37. no Jwt (session-based, not JWT)
    #[test]
    fn web_lc_37_no_jwt() {
        assert!(!make_auth_login_controller("my_app").contains("Jwt"));
    }
    // web_38. no Json response (redirects only)
    #[test]
    fn web_lc_38_no_json_response_type() {
        assert!(!make_auth_login_controller("my_app").contains("Json(json!"));
    }
    // web_39. uses axum::extract::Form
    #[test]
    fn web_lc_39_uses_axum_form() {
        assert!(make_auth_login_controller("my_app").contains("Form"));
    }
    // web_40. uses Redirect
    #[test]
    fn web_lc_40_uses_redirect() {
        assert!(make_auth_login_controller("my_app").contains("Redirect"));
    }
    // web_41. uses IntoResponse
    #[test]
    fn web_lc_41_uses_into_response() {
        assert!(make_auth_login_controller("my_app").contains("IntoResponse"));
    }
    // web_42. store uses use validator::Validate inline
    #[test]
    fn web_lc_42_store_uses_validator_validate_inline() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("use validator::Validate"));
    }
    // web_43. imports from crate::app::http::requests::login_request
    #[test]
    fn web_lc_43_imports_from_requests_module() {
        assert!(make_auth_login_controller("my_app")
            .contains("crate::app::http::requests::login_request"));
    }
    // web_44. uses crate name in runtime import
    #[test]
    fn web_lc_44_uses_crate_name_in_import() {
        let out = make_auth_login_controller("my_crate");
        assert!(out.contains("use ::my_crate::"));
    }
    // web_45. store returns impl IntoResponse (not Result)
    #[test]
    fn web_lc_45_store_returns_impl_into_response() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        let destroy_pos = out.find("pub async fn destroy(").unwrap();
        let store_sig = &out[store_pos..destroy_pos];
        assert!(store_sig.contains("impl IntoResponse"));
        assert!(!store_sig.contains("Result<impl IntoResponse"));
    }
    // web_46. destroy accepts session parameter
    #[test]
    fn web_lc_46_destroy_accepts_session() {
        let out = make_auth_login_controller("my_app");
        let destroy_pos = out.find("pub async fn destroy(").unwrap();
        assert!(out[destroy_pos..].contains("Session"));
    }
    // web_47. destroy has no database access
    #[test]
    fn web_lc_47_destroy_has_no_db_access() {
        let out = make_auth_login_controller("my_app");
        let destroy_pos = out.find("pub async fn destroy(").unwrap();
        assert!(!out[destroy_pos..].contains("sqlx"));
        assert!(!out[destroy_pos..].contains("db"));
    }
    // web_48. show accepts Session extractor
    #[test]
    fn web_lc_48_show_accepts_session() {
        let out = make_auth_login_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[show_pos..store_pos].contains("Session"));
    }
    // web_49. store accepts Session extractor
    #[test]
    fn web_lc_49_store_accepts_session() {
        let out = make_auth_login_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        let destroy_pos = out.find("pub async fn destroy(").unwrap();
        assert!(out[store_pos..destroy_pos].contains("Session"));
    }
    // web_50. no Japanese text
    #[test]
    fn web_lc_50_no_japanese_text() {
        let out = make_auth_login_controller("my_app");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c)
            || ('\u{4E00}'..='\u{9FFF}').contains(&c)));
    }

    // ── exception_handler_rs: API-path returns JSON, not HTML (25) ────────────

    // eh_01. expects_json function is present
    #[test]
    fn eh_01_expects_json_fn_present() {
        assert!(exception_handler_rs("my_app").contains("fn expects_json("));
    }
    // eh_02. render function is present
    #[test]
    fn eh_02_render_fn_present() {
        assert!(exception_handler_rs("my_app").contains("pub async fn render("));
    }
    // eh_03. /api/ path check is present inside expects_json
    #[test]
    fn eh_03_api_path_check_present() {
        let out = exception_handler_rs("my_app");
        let fn_pos = out.find("fn expects_json(").unwrap();
        let fn_end = out[fn_pos..].find("\n}").unwrap() + fn_pos;
        assert!(out[fn_pos..fn_end].contains("/api/"),
            "expects_json must return true for /api/ paths");
    }
    // eh_04. /api/ path check uses starts_with
    #[test]
    fn eh_04_api_path_uses_starts_with() {
        let out = exception_handler_rs("my_app");
        let fn_pos = out.find("fn expects_json(").unwrap();
        let fn_end = out[fn_pos..].find("\n}").unwrap() + fn_pos;
        assert!(out[fn_pos..fn_end].contains("starts_with(\"/api/\")"),
            "must use starts_with to match /api/ prefix");
    }
    // eh_05. /api/ path check appears before Accept header check
    #[test]
    fn eh_05_api_path_check_before_accept_check() {
        let out = exception_handler_rs("my_app");
        let api_pos = out.find("starts_with(\"/api/\")").unwrap();
        let accept_pos = out.find("ACCEPT").unwrap();
        assert!(api_pos < accept_pos, "/api/ check must come before Accept header check");
    }
    // eh_06. /api/ path check returns true immediately
    #[test]
    fn eh_06_api_path_check_returns_true() {
        let out = exception_handler_rs("my_app");
        let api_pos = out.find("starts_with(\"/api/\")").unwrap();
        assert!(out[api_pos..api_pos + 60].contains("return true"),
            "must return true immediately for /api/ paths");
    }
    // eh_07. checks application/json Accept header
    #[test]
    fn eh_07_checks_application_json_accept() {
        assert!(exception_handler_rs("my_app").contains("application/json"));
    }
    // eh_08. checks +json Accept header suffix
    #[test]
    fn eh_08_checks_plus_json_accept() {
        assert!(exception_handler_rs("my_app").contains("+json"));
    }
    // eh_09. checks /json Accept header
    #[test]
    fn eh_09_checks_slash_json_accept() {
        assert!(exception_handler_rs("my_app").contains("\"/json\""));
    }
    // eh_10. checks Content-Type header for JSON
    #[test]
    fn eh_10_checks_content_type_header() {
        assert!(exception_handler_rs("my_app").contains("CONTENT_TYPE"));
    }
    // eh_11. checks X-Requested-With header (AJAX detection)
    #[test]
    fn eh_11_checks_x_requested_with() {
        assert!(exception_handler_rs("my_app").contains("x-requested-with"));
    }
    // eh_12. checks XMLHttpRequest value
    #[test]
    fn eh_12_checks_xmlhttprequest_value() {
        assert!(exception_handler_rs("my_app").contains("xmlhttprequest"));
    }
    // eh_13. handles empty Accept as accepts-any
    #[test]
    fn eh_13_empty_accept_is_accepts_any() {
        let out = exception_handler_rs("my_app");
        assert!(out.contains("accept.is_empty()") || out.contains("is_empty()"));
    }
    // eh_14. falls back to errors.generic template
    #[test]
    fn eh_14_falls_back_to_errors_generic() {
        assert!(exception_handler_rs("my_app").contains("errors.generic"));
    }
    // eh_15. renders errors.{code} template
    #[test]
    fn eh_15_renders_error_code_template() {
        assert!(exception_handler_rs("my_app").contains("errors."));
    }
    // eh_16. passes status code to template context
    #[test]
    fn eh_16_passes_code_to_context() {
        let out = exception_handler_rs("my_app");
        let render_pos = out.find("pub async fn render(").unwrap();
        assert!(out[render_pos..].contains("code     =>"));
    }
    // eh_17. passes message to template context
    #[test]
    fn eh_17_passes_message_to_context() {
        let out = exception_handler_rs("my_app");
        let render_pos = out.find("pub async fn render(").unwrap();
        assert!(out[render_pos..].contains("message  =>"));
    }
    // eh_18. returns original response when no error template found
    #[test]
    fn eh_18_returns_original_response_on_missing_template() {
        let out = exception_handler_rs("my_app");
        assert!(out.contains("response") && out.contains("return response"));
    }
    // eh_19. skips rendering for non-error responses
    #[test]
    fn eh_19_skips_non_error_responses() {
        let out = exception_handler_rs("my_app");
        assert!(out.contains("is_client_error") || out.contains("is_server_error"));
    }
    // eh_20. imports Html for HTML response
    #[test]
    fn eh_20_imports_html() {
        assert!(exception_handler_rs("my_app").contains("Html"));
    }
    // eh_21. imports IntoResponse
    #[test]
    fn eh_21_imports_into_response() {
        assert!(exception_handler_rs("my_app").contains("IntoResponse"));
    }
    // eh_22. imports State extractor
    #[test]
    fn eh_22_imports_state() {
        assert!(exception_handler_rs("my_app").contains("State"));
    }
    // eh_23. no #[path] attribute
    #[test]
    fn eh_23_no_path_attribute() {
        assert!(!exception_handler_rs("my_app").contains("#[path"));
    }
    // eh_24. uses provided crate name in AppState import
    #[test]
    fn eh_24_uses_crate_name_for_app_state() {
        assert!(exception_handler_rs("my_crate").contains("use ::my_crate::AppState"));
    }
    // eh_25. no Japanese text
    #[test]
    fn eh_25_no_japanese_text() {
        let out = exception_handler_rs("my_app");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c)
            || ('\u{4E00}'..='\u{9FFF}').contains(&c)));
    }

    // ── make_auth_register_controller (15) ────────────────────────────────────

    // rc_01. regression: snake_case module import
    #[test]
    fn rc_01_imports_snake_case_register_request() {
        assert!(make_auth_register_controller("my_app").contains("register_request::RegisterRequest"));
    }
    // rc_02. regression: PascalCase module path banned
    #[test]
    fn rc_02_no_pascal_case_register_request_import() {
        assert!(!make_auth_register_controller("my_app").contains("RegisterRequest::RegisterRequest"));
    }
    // rc_03. regression: #[path] banned
    #[test]
    fn rc_03_no_path_attribute() {
        assert!(!make_auth_register_controller("my_app").contains("#[path"));
    }
    // rc_04. security: password must not be stored in flash
    #[test]
    fn rc_04_password_not_in_flash() {
        assert!(!make_auth_register_controller("my_app").contains("flash_old_password"));
    }
    // rc_05. behavior: show() reads AND forgets flash_old_name
    #[test]
    fn rc_05_show_reads_and_forgets_flash_old_name() {
        let out = make_auth_register_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        let show_body = &out[show_pos..store_pos];
        assert!(show_body.contains("get(\"flash_old_name\")") && show_body.contains("forget(\"flash_old_name\")"));
    }
    // rc_06. behavior: show() reads AND forgets flash_old_email
    #[test]
    fn rc_06_show_reads_and_forgets_flash_old_email() {
        let out = make_auth_register_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        let show_body = &out[show_pos..store_pos];
        assert!(show_body.contains("get(\"flash_old_email\")") && show_body.contains("forget(\"flash_old_email\")"));
    }
    // rc_07. behavior: show() renders correct view name
    #[test]
    fn rc_07_show_renders_auth_register_view() {
        let out = make_auth_register_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[show_pos..store_pos].contains("\"auth.register\""));
    }
    // rc_08. behavior: show() passes all 3 context vars
    #[test]
    fn rc_08_show_passes_three_context_vars() {
        let out = make_auth_register_controller("my_app");
        let show_pos = out.find("pub async fn show(").unwrap();
        let store_pos = out.find("pub async fn store(").unwrap();
        let show_body = &out[show_pos..store_pos];
        assert!(show_body.contains("flash_error =>") && show_body.contains("old_name =>") && show_body.contains("old_email =>"));
    }
    // rc_09. behavior: validation error redirects to /register
    #[test]
    fn rc_09_validation_error_redirects_to_register() {
        let out = make_auth_register_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("Redirect::to(\"/register\")"));
    }
    // rc_10. behavior: success redirects to /login not /dashboard
    #[test]
    fn rc_10_success_redirects_to_login() {
        let out = make_auth_register_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].contains("Redirect::to(\"/login\")"));
    }
    // rc_11. behavior: flash_old_name is set on multiple error paths
    #[test]
    fn rc_11_flash_old_name_set_on_multiple_error_paths() {
        let out = make_auth_register_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        assert!(out[store_pos..].matches("put(\"flash_old_name\"").count() >= 2);
    }
    // rc_12. behavior: email duplicate uses constraint name
    #[test]
    fn rc_12_email_duplicate_uses_constraint_name() {
        assert!(make_auth_register_controller("my_app").contains("users_email_key"));
    }
    // rc_13. behavior: email duplicate returns Conflict or specific message
    #[test]
    fn rc_13_email_duplicate_handled_specifically() {
        let out = make_auth_register_controller("my_app");
        assert!(out.contains("already registered") || out.contains("AppError::Conflict"));
    }
    // rc_14. behavior: validation runs before DB access
    #[test]
    fn rc_14_validation_before_db_access() {
        let out = make_auth_register_controller("my_app");
        let store_pos = out.find("pub async fn store(").unwrap();
        let store_body = &out[store_pos..];
        let validate_pos = store_body.find("req.validate()").unwrap();
        let sqlx_pos = store_body.find("sqlx::query_as").unwrap();
        assert!(validate_pos < sqlx_pos, "validate() must appear before sqlx::query_as");
    }
    // rc_15. regression: no Japanese text
    #[test]
    fn rc_15_no_japanese_text() {
        let out = make_auth_register_controller("my_app");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c)
            || ('\u{4E00}'..='\u{9FFF}').contains(&c)));
    }

    // ── make_auth_api_register_controller (15) ────────────────────────────────

    // arc_01. regression: snake_case import
    #[test]
    fn arc_01_imports_snake_case_register_request() {
        assert!(make_auth_api_register_controller("my_app").contains("register_request::RegisterRequest"));
    }
    // arc_02. regression: PascalCase import banned
    #[test]
    fn arc_02_no_pascal_case_register_request() {
        assert!(!make_auth_api_register_controller("my_app").contains("RegisterRequest::RegisterRequest"));
    }
    // arc_03. regression: #[path] banned
    #[test]
    fn arc_03_no_path_attribute() {
        assert!(!make_auth_api_register_controller("my_app").contains("#[path"));
    }
    // arc_04. regression: API has no session
    #[test]
    fn arc_04_no_session_calls() {
        assert!(!make_auth_api_register_controller("my_app").contains("session."));
    }
    // arc_05. regression: API does not redirect
    #[test]
    fn arc_05_no_redirect() {
        assert!(!make_auth_api_register_controller("my_app").contains("Redirect::"));
    }
    // arc_06. regression: API does not call view()
    #[test]
    fn arc_06_no_view_call() {
        assert!(!make_auth_api_register_controller("my_app").contains("view("));
    }
    // arc_07. behavior: uses ValidatedJson extractor
    #[test]
    fn arc_07_uses_validated_json() {
        assert!(make_auth_api_register_controller("my_app")
            .contains("ValidatedJson(req): ValidatedJson<RegisterRequest>"));
    }
    // arc_08. behavior: Hash::make uses ? for error propagation
    #[test]
    fn arc_08_hash_make_uses_question_mark() {
        assert!(make_auth_api_register_controller("my_app").contains("Hash::make(&req.password)?"));
    }
    // arc_09. behavior: success returns 201 CREATED
    #[test]
    fn arc_09_returns_status_created() {
        assert!(make_auth_api_register_controller("my_app").contains("StatusCode::CREATED"));
    }
    // arc_10. behavior: response includes token
    #[test]
    fn arc_10_response_includes_token() {
        assert!(make_auth_api_register_controller("my_app").contains("\"token\""));
    }
    // arc_11. behavior: email duplicate ↁEConflict with message
    #[test]
    fn arc_11_email_duplicate_is_conflict() {
        let out = make_auth_api_register_controller("my_app");
        assert!(out.contains("users_email_key") && out.contains("AppError::Conflict"));
    }
    // arc_12. behavior: JWT issued via Jwt::encode_with_config
    #[test]
    fn arc_12_jwt_encode_called() {
        assert!(make_auth_api_register_controller("my_app").contains("Jwt::encode_with_config(u.id as i64, &ctx.state.config.jwt)?"));
    }
    // arc_13. security: response does not include password
    #[test]
    fn arc_13_response_excludes_password() {
        assert!(!make_auth_api_register_controller("my_app").contains("u.password"));
    }
    // arc_14. behavior: inserts into users table
    #[test]
    fn arc_14_inserts_into_users_table() {
        assert!(make_auth_api_register_controller("my_app").contains("INSERT INTO users"));
    }
    // arc_15. regression: no Japanese text
    #[test]
    fn arc_15_no_japanese_text() {
        let out = make_auth_api_register_controller("my_app");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c)
            || ('\u{4E00}'..='\u{9FFF}').contains(&c)));
    }

    // -- make_auth_api password reset + email verification --------------------

    #[test]
    fn api_parity_01_route_use_decl_has_new_controllers() {
        let out = auth_api_route_use_decl();
        for module in &[
            "api_forgot_password_controller",
            "api_reset_password_controller",
            "api_verify_email_controller",
        ] {
            assert!(out.contains(module), "missing {}", module);
        }
    }

    #[test]
    fn api_parity_02_route_snippet_has_reset_and_verification_routes() {
        let out = auth_api_route_snippet();
        for route in &[
            "/api/auth/forgot-password",
            "/api/auth/reset-password",
            "/api/auth/email/verify/{id}/{hash}",
            "/api/auth/email/verification-notification",
        ] {
            assert!(out.contains(route), "missing {}", route);
        }
    }

    #[test]
    fn api_parity_03_forgot_password_controller_is_json_only() {
        let out = make_auth_api_forgot_password_controller("my_app");
        assert!(out.contains("ValidatedJson(req): ValidatedJson<ForgotPasswordRequest>"));
        assert!(out.contains("Json(json!"));
        assert!(out.contains("INSERT INTO password_reset_tokens"));
        assert!(out.contains("Throttle::too_many"));
        assert!(out.contains("random_token()"));
        assert!(!out.contains("Redirect::"));
        assert!(!out.contains("view("));
    }

    #[test]
    fn api_parity_04_reset_password_controller_updates_and_clears_token() {
        let out = make_auth_api_reset_password_controller("my_app");
        assert!(out.contains("ValidatedJson(req): ValidatedJson<ResetPasswordRequest>"));
        assert!(out.contains("Hash::check"));
        assert!(out.contains("Duration::minutes(TOKEN_TTL_MINUTES)"));
        assert!(out.contains("UPDATE users SET password"));
        assert!(out.contains("DELETE FROM password_reset_tokens"));
        assert!(out.contains("AppError::Http(422"));
        assert!(!out.contains("Redirect::"));
        assert!(!out.contains("view("));
    }

    #[test]
    fn api_parity_05_verify_email_controller_is_signed_json_flow() {
        let out = make_auth_api_verify_email_controller("my_app");
        assert!(out.contains("pub async fn verify"));
        assert!(out.contains("pub async fn resend"));
        assert!(out.contains("send_verification_email"));
        assert!(out.contains("email_verification_hash"));
        assert!(out.contains("verify_signature(&payload"));
        assert!(out.contains("sign(&payload"));
        assert!(out.contains("UPDATE users SET email_verified_at = NOW()"));
        assert!(out.contains("Json(json!"));
        assert!(out.contains("JwtUser"));
        assert!(out.contains("Throttle::too_many"));
        assert!(!out.contains("Redirect::"));
        assert!(!out.contains("view("));
    }

    #[test]
    fn api_parity_06_api_register_sends_verification_email() {
        let out = make_auth_api_register_controller("my_app");
        assert!(out.contains("api_verify_email_controller::send_verification_email"));
    }

    #[test]
    fn api_parity_07_new_api_templates_have_no_japanese_text() {
        for out in &[
            make_auth_api_forgot_password_controller("my_app"),
            make_auth_api_reset_password_controller("my_app"),
            make_auth_api_verify_email_controller("my_app"),
        ] {
            assert!(!out.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c)
                || ('\u{4E00}'..='\u{9FFF}').contains(&c)));
        }
    }

    // ── make_auth_dashboard_controller (8) ────────────────────────────────────

    // dc_01. regression: #[path] banned
    #[test]
    fn dc_01_no_path_attribute() {
        assert!(!make_auth_dashboard_controller("my_app").contains("#[path"));
    }
    // dc_02. behavior: guarded by the VerifiedUser extractor, not raw Session
    #[test]
    fn dc_02_uses_verified_user_extractor() {
        let out = make_auth_dashboard_controller("my_app");
        assert!(out.contains("VerifiedUser") && !out.contains("session."));
    }
    // dc_03. behavior: no DB access needed
    #[test]
    fn dc_03_no_db_access() {
        assert!(!make_auth_dashboard_controller("my_app").contains("sqlx"));
    }
    // dc_04. behavior: correct view name "dashboard"
    #[test]
    fn dc_04_renders_dashboard_view() {
        assert!(make_auth_dashboard_controller("my_app").contains("\"dashboard\""));
    }
    // dc_05. behavior: passes user_id from the verified user
    #[test]
    fn dc_05_passes_user_id_from_auth() {
        assert!(make_auth_dashboard_controller("my_app").contains("user_id => user.id"));
    }
    // dc_06. behavior: returns Result<impl IntoResponse, AppError>
    #[test]
    fn dc_06_returns_result() {
        assert!(make_auth_dashboard_controller("my_app").contains("Result<impl IntoResponse, AppError>"));
    }
    // dc_07. security: no session mutation
    #[test]
    fn dc_07_no_session_calls() {
        assert!(!make_auth_dashboard_controller("my_app").contains("session."));
    }
    // dc_08. regression: no Japanese text
    #[test]
    fn dc_08_no_japanese_text() {
        let out = make_auth_dashboard_controller("my_app");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c)
            || ('\u{4E00}'..='\u{9FFF}').contains(&c)));
    }

    // ── view_auth_login (10) ──────────────────────────────────────────────────

    // vl_01. regression: form action must be /login not /register
    #[test]
    fn vl_01_form_action_is_login() {
        let out = view_auth_login();
        assert!(out.contains("action=\"/login\"") && !out.contains("action=\"/register\""));
    }
    // vl_02. regression: login form has no name field
    #[test]
    fn vl_02_no_name_field() {
        assert!(!view_auth_login().contains("name=\"name\""));
    }
    // vl_03. regression: correct autocomplete for password
    #[test]
    fn vl_03_autocomplete_current_password() {
        let out = view_auth_login();
        assert!(out.contains("autocomplete=\"current-password\"")
            && !out.contains("autocomplete=\"new-password\""));
    }
    // vl_04. behavior: old_email preserved as value attribute
    #[test]
    fn vl_04_old_email_as_value_attribute() {
        assert!(view_auth_login().contains("value=\"{{ old_email }}\""));
    }
    // vl_05. behavior: flash_error conditionally shown
    #[test]
    fn vl_05_flash_error_conditional() {
        assert!(view_auth_login().contains("{% if flash_error %}"));
    }
    // vl_06. behavior: password input type is password not text
    #[test]
    fn vl_06_password_type_is_password() {
        assert!(view_auth_login().contains("type=\"password\""));
    }
    // vl_07. behavior: email input name attribute
    #[test]
    fn vl_07_email_input_name() {
        assert!(view_auth_login().contains("name=\"email\""));
    }
    // vl_08. behavior: password input name attribute
    #[test]
    fn vl_08_password_input_name() {
        assert!(view_auth_login().contains("name=\"password\""));
    }
    // vl_09. behavior: link to register page
    #[test]
    fn vl_09_link_to_register() {
        assert!(view_auth_login().contains("/register"));
    }
    // vl_10. behavior: extends correct layout
    #[test]
    fn vl_10_extends_layouts_app() {
        assert!(view_auth_login().contains("{% extends \"layouts.app\" %}"));
    }

    // ── view_auth_register (12) ───────────────────────────────────────────────

    // vr_01. regression: form action must be /register not /login
    #[test]
    fn vr_01_form_action_is_register() {
        let out = view_auth_register();
        assert!(out.contains("action=\"/register\"") && !out.contains("action=\"/login\""));
    }
    // vr_02. regression: correct autocomplete for password
    #[test]
    fn vr_02_autocomplete_new_password() {
        let out = view_auth_register();
        assert!(out.contains("autocomplete=\"new-password\"")
            && !out.contains("autocomplete=\"current-password\""));
    }
    // vr_03. behavior: old_name preserved
    #[test]
    fn vr_03_old_name_value_attribute() {
        assert!(view_auth_register().contains("value=\"{{ old_name }}\""));
    }
    // vr_04. behavior: old_email preserved
    #[test]
    fn vr_04_old_email_value_attribute() {
        assert!(view_auth_register().contains("value=\"{{ old_email }}\""));
    }
    // vr_05. behavior: flash_error conditionally shown
    #[test]
    fn vr_05_flash_error_conditional() {
        assert!(view_auth_register().contains("{% if flash_error %}"));
    }
    // vr_06. behavior: name input exists
    #[test]
    fn vr_06_name_input_present() {
        assert!(view_auth_register().contains("name=\"name\""));
    }
    // vr_07. behavior: email input exists
    #[test]
    fn vr_07_email_input_present() {
        assert!(view_auth_register().contains("name=\"email\""));
    }
    // vr_08. behavior: password input exists
    #[test]
    fn vr_08_password_input_present() {
        assert!(view_auth_register().contains("name=\"password\""));
    }
    // vr_09. behavior: password input type is password
    #[test]
    fn vr_09_password_type_is_password() {
        assert!(view_auth_register().contains("type=\"password\""));
    }
    // vr_10. behavior: link back to login
    #[test]
    fn vr_10_link_to_login() {
        assert!(view_auth_register().contains("/login"));
    }
    // vr_11. behavior: extends correct layout
    #[test]
    fn vr_11_extends_layouts_app() {
        assert!(view_auth_register().contains("{% extends \"layouts.app\" %}"));
    }
    // vr_12. behavior: old values rendered as value= attributes (input pre-fill)
    #[test]
    fn vr_12_old_values_as_value_attributes() {
        let out = view_auth_register();
        assert!(out.contains("value=\"{{ old_name }}\"") || out.contains("value=\"{{ old_email }}\""));
    }

    // ── view_auth_dashboard (5) ───────────────────────────────────────────────

    // vd_01. regression: logout form action must be /logout
    #[test]
    fn vd_01_logout_form_action_is_logout() {
        let out = view_auth_dashboard();
        assert!(out.contains("action=\"/logout\"")
            && !out.contains("action=\"/login\"")
            && !out.contains("action=\"/dashboard\""));
    }
    // vd_02. behavior: user_id is displayed
    #[test]
    fn vd_02_displays_user_id() {
        assert!(view_auth_dashboard().contains("{{ user_id }}"));
    }
    // vd_03. behavior: logout uses POST method
    #[test]
    fn vd_03_logout_uses_post() {
        let out = view_auth_dashboard();
        let logout_pos = out.find("/logout").unwrap();
        let surrounding = &out[logout_pos.saturating_sub(60)..logout_pos + 10];
        assert!(surrounding.contains("POST") || out.contains("method=\"POST\""));
    }
    // vd_04. behavior: logout is a button not a link
    #[test]
    fn vd_04_logout_is_submit_button() {
        assert!(view_auth_dashboard().contains("<button type=\"submit\">"));
    }
    // vd_05. behavior: extends correct layout
    #[test]
    fn vd_05_extends_layouts_app() {
        assert!(view_auth_dashboard().contains("{% extends \"layouts.app\" %}"));
    }

    // ── user_model_rs (10) ────────────────────────────────────────────────────

    // um_01. security: password field has skip_serializing
    #[test]
    fn um_01_password_has_skip_serializing() {
        assert!(user_model_rs().contains("#[serde(skip_serializing, default)]"));
    }
    // um_02. security: password field exists for DB reads
    #[test]
    fn um_02_password_field_exists() {
        assert!(user_model_rs().contains("pub password: String"));
    }
    // um_03. behavior: find_by_email uses parameterized query
    #[test]
    fn um_03_find_by_email_uses_parameter() {
        assert!(user_model_rs().contains("$1"));
    }
    // um_04. behavior: find_by_email returns single result
    #[test]
    fn um_04_find_by_email_limits_to_one() {
        let out = user_model_rs();
        assert!(out.contains("LIMIT 1") || out.contains("fetch_optional"));
    }
    // um_05. behavior: id field type
    #[test]
    fn um_05_id_is_i32() {
        assert!(user_model_rs().contains("pub id: i32"));
    }
    // um_06. behavior: created_at field type
    #[test]
    fn um_06_created_at_is_datetime_utc() {
        assert!(user_model_rs().contains("DateTime<Utc>"));
    }
    // um_07. behavior: derives FromRow for sqlx
    #[test]
    fn um_07_derives_from_row() {
        assert!(user_model_rs().contains("FromRow"));
    }
    // um_08. behavior: derives Serialize (password skipped, rest serialized)
    #[test]
    fn um_08_derives_serialize() {
        assert!(user_model_rs().contains("Serialize"));
    }
    // um_09. behavior: find_by_email returns Option
    #[test]
    fn um_09_find_by_email_returns_option() {
        assert!(user_model_rs().contains("Result<Option<Self>, sqlx::Error>"));
    }
    // um_10. behavior: WHERE clause filters by email
    #[test]
    fn um_10_where_clause_filters_by_email() {
        assert!(user_model_rs().contains("WHERE email"));
    }
    #[test]
    fn um_11_user_model_has_no_auth_verification_column() {
        let out = user_model_rs();
        assert!(!out.contains("email_verified_at"));
        assert!(!out.contains("is_verified"));
    }
    #[test]
    fn um_12_user_model_avoids_select_star() {
        assert!(!user_model_rs().contains("SELECT * FROM users"));
    }

    // ── user_controller (10) ──────────────────────────────────────────────────

    // uc_01. behavior: cache key is consistent between remember and forget
    #[test]
    fn uc_01_cache_key_consistent() {
        let out = user_controller("my_app");
        assert!(out.contains("Cache::remember") && out.contains("Cache::forget"));
        assert!(out.matches("\"users.all\"").count() >= 2);
    }
    // uc_02. behavior: cache TTL is 60 seconds
    #[test]
    fn uc_02_cache_ttl_60_seconds() {
        assert!(user_controller("my_app").contains("Duration::from_secs(60)"));
    }
    // uc_03. behavior: store() returns 201 CREATED not 200
    #[test]
    fn uc_03_store_returns_created() {
        assert!(user_controller("my_app").contains("StatusCode::CREATED"));
    }
    // uc_04. behavior: email duplicate uses specific constraint name
    #[test]
    fn uc_04_email_duplicate_uses_constraint_name() {
        assert!(user_controller("my_app").contains("users_email_key"));
    }
    // uc_05. behavior: email duplicate returns Conflict with message
    #[test]
    fn uc_05_email_duplicate_is_conflict() {
        assert!(user_controller("my_app").contains("AppError::Conflict(\"Email already taken.\""));
    }
    // uc_06. behavior: mock function has no DB or cache access
    #[test]
    fn uc_06_mock_has_no_db_or_cache() {
        let out = user_controller("my_app");
        let mock_pos = out.find("pub async fn mock(").unwrap();
        let mock_body = &out[mock_pos..];
        assert!(!mock_body.contains("sqlx::query") && !mock_body.contains("Cache::"));
    }
    // uc_07. behavior: mock returns static sample data
    #[test]
    fn uc_07_mock_has_static_sample_data() {
        let out = user_controller("my_app");
        assert!(out.contains("Alice") || out.contains("Bob") || out.contains("Carol"));
    }
    // uc_08. regression: StoreUserRequest validates password length
    #[test]
    fn uc_08_password_validate_min_length() {
        assert!(user_controller("my_app").contains("#[validate(length(min = 8"));
    }
    // uc_09. regression: StoreUserRequest validates email format
    #[test]
    fn uc_09_email_validate_format() {
        assert!(user_controller("my_app").contains("#[validate(email"));
    }
    // uc_10. regression: user_controller has no destroy function
    #[test]
    fn uc_10_no_destroy_function() {
        assert!(!user_controller("my_app").contains("pub async fn destroy("));
    }
    #[test]
    fn uc_11_store_hashes_password_before_insert() {
        let out = user_controller("my_app");
        assert!(out.contains("Hash::make(&req.password)?"));
        assert!(out.contains(".bind(&hashed)"));
        assert!(!out.contains(".bind(&req.password)"));
    }
    #[test]
    fn uc_12_store_imports_hash_facade() {
        assert!(user_controller("my_app").contains("Context, Hash, ValidatedJson"));
    }

    // ── routes / bootstrap / main_rs (5) ─────────────────────────────────────

    // misc_01. routes_api: auth routes absent (injected separately by make:auth --api)
    #[test]
    fn misc_01_routes_api_has_no_auth_routes() {
        let out = routes_api("my_app");
        assert!(!out.contains("\"/login\"") && !out.contains("\"/register\""));
    }
    // misc_02. routes_web: auth routes absent (injected separately by make:auth)
    #[test]
    fn misc_02_routes_web_has_no_auth_routes() {
        let out = routes_web("my_app");
        assert!(!out.contains("\"/login\"") && !out.contains("\"/register\""));
    }
    // misc_03. bootstrap_lib_rs: auth types re-exported
    #[test]
    fn misc_03_bootstrap_exports_auth_types() {
        let out = bootstrap_lib_rs();
        assert!(out.contains("Hash") && out.contains("Session") && out.contains("Auth") && out.contains("AuthUser"));
    }
    // misc_04. bootstrap_lib_rs: middleware re-exported
    #[test]
    fn misc_04_bootstrap_exports_middleware() {
        let out = bootstrap_lib_rs();
        assert!(out.contains("authenticate") && out.contains("session_middleware"));
    }
    // misc_05. main_rs: binds to 0.0.0.0 not 127.0.0.1
    #[test]
    fn misc_05_main_binds_to_all_interfaces() {
        let out = main_rs("my_app");
        assert!(out.contains("0.0.0.0:3000") && !out.contains("127.0.0.1:3000"));
    }
}
