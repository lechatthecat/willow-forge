# Willow Forge

A Laravel-inspired web framework for Rust.

---

## Creating a new app

Install the CLI from the repo root:

```bash
cargo install --path .
```

Scaffold a new application:

```bash
willow-forge new my-app
cd my-app
cargo run
```

The server starts on `http://localhost:3000`.

Run without installing:

```bash
cargo run --manifest-path /path/to/willow/Cargo.toml -- new my-app
```

---

## Generated app layout

```text
my-app/
├── src/
│   ├── main.rs
│   ├── middleware.rs               ← global / api / web middleware groups
│   ├── app/
│   │   ├── mod.rs
│   │   ├── Http/
│   │   │   ├── mod.rs
│   │   │   ├── Controllers/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── HomeController.rs
│   │   │   │   ├── UserController.rs
│   │   │   │   └── StatusController.rs
│   │   │   ├── Middleware/
│   │   │   │   ├── mod.rs
│   │   │   │   └── LogRequest.rs
│   │   │   └── Requests/
│   │   │       ├── mod.rs
│   │   │       └── StoreUserRequest.rs
│   │   ├── Models/
│   │   │   ├── mod.rs
│   │   │   └── User.rs
│   │   └── Exceptions/
│   │       ├── mod.rs
│   │       └── Handler.rs
│   └── routes/
│       ├── mod.rs
│       ├── web.rs
│       └── api.rs
├── bootstrap/
│   ├── lib.rs                      ← library root, bootstrap() lives here
│   └── app_service_provider.rs
├── config/
│   ├── app.toml
│   ├── auth.toml
│   ├── cache.toml
│   └── database.toml
├── database/
│   └── migrations/
├── resources/
│   └── views/
│       ├── layouts/
│       │   └── app.jinja.html
│       ├── errors/
│       │   ├── 404.jinja.html
│       │   ├── 500.jinja.html
│       │   └── generic.jinja.html
│       └── welcome.jinja.html
├── docker/
│   └── docker-compose.yml
├── .env
└── Cargo.toml
```

All Rust source files live under `src/`. There are no `#[path]` attributes anywhere — the standard Rust module system is used throughout.

---

## Routing

Routes live in `src/routes/web.rs` (HTML) and `src/routes/api.rs` (JSON).

Each file returns an `axum::Router<Arc<AppState>>`:

```rust
// src/routes/web.rs
use crate::app::Http::Controllers::HomeController;
use axum::{routing::get, Router};
use std::sync::Arc;
use my_app::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(HomeController::index))
}

// src/routes/api.rs
use crate::app::Http::Controllers::{UserController, StatusController};
use axum::{routing::get, Router};
use std::sync::Arc;
use my_app::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/users", get(UserController::index).post(UserController::store))
        .route("/api/status", get(StatusController::index))
}
```

Both routers are merged in `src/main.rs` with middleware applied:

```rust
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
```

---

## AppState and Context (dependency injection)

Willow Forge does DI via `Arc<AppState>` passed through the axum router state.

### AppState

```rust
pub struct AppState {
    pub config: Config,
    pub services: Services,  // PgPool is Arc-based internally
    pub views: ViewEngine,   // MiniJinja Environment is Arc-based internally
}
```

### Context extractor

`Context` is an axum extractor that pulls `Arc<AppState>` out of the router state.
Add it as the first parameter of any handler:

```rust
pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {
    let app_name = &ctx.state.config.app_name;
    Ok(Json(json!({ "app": app_name })))
}
```

### Bootstrap

`bootstrap/lib.rs` wires everything together at startup:

1. Reads `.env`
2. Builds `Config` from environment variables
3. Initialises the view engine from `resources/views/`
4. Creates the database pool and Redis cluster client via `app_service_provider`
5. Returns `Arc<AppState>`

---

## Error handling

`AppError` is defined in `willow-forge-runtime` and re-exported from `bootstrap/lib.rs`:

```rust
pub enum AppError {
    NotFound,                          // 404
    Unauthorized,                      // 401
    Forbidden,                         // 403
    Validation(ValidationError),       // 422
    Conflict(String),                  // 409
    ServiceUnavailable,                // 503
    TooManyRequests,                   // 429
    Http(u16, String),                 // any status code
    View(ViewError),                   // 500
    Database(sqlx::Error),             // 500
    Redis(redis::RedisError),          // 500
    Internal,                          // 500
}
```

Controllers return `Result<impl IntoResponse, AppError>`. `From` impls let `?` propagate errors automatically:

```rust
// ViewError → AppError::View via ?
pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {
    Ok(view(&ctx, "welcome", context! { ... })?)
}

// sqlx::Error → AppError::Database via ?
let users = sqlx::query_as::<_, User>(...).fetch_all(pool).await?;

// Known conflict → AppError::Conflict
.map_err(|e| match e {
    sqlx::Error::Database(ref db) if db.constraint() == Some("users_email_key")
        => AppError::Conflict("Email already taken.".to_string()),
    other => AppError::Database(other),
})?
```

### Error response table

| AppError variant | HTTP status | Body |
| --- | --- | --- |
| `NotFound` | 404 | `{"message":"Not found"}` |
| `Unauthorized` | 401 | `{"message":"Unauthorized"}` |
| `Forbidden` | 403 | `{"message":"Forbidden"}` |
| `Validation` | 422 | `{"message":"The given data was invalid.","errors":{...}}` |
| `Conflict(msg)` | 409 | `{"message":"<msg>"}` |
| `ServiceUnavailable` | 503 | `{"message":"Service unavailable"}` |
| `TooManyRequests` | 429 | `{"message":"Too many requests"}` |
| `Http(code, msg)` | `code` | `{"message":"<msg>"}` |
| `View` / `Database` / `Redis` / `Internal` | 500 | `{"message":"Internal server error"}` |

### HTML error views

For browser requests, the exception handler in `src/app/Exceptions/Handler.rs` automatically renders HTML views from `resources/views/errors/`:

- `errors/404.jinja.html` — 404 specific
- `errors/500.jinja.html` — 500 specific
- `errors/generic.jinja.html` — fallback for all other codes

Variables available: `code`, `message`, `app_name`, `app_env`.

### JSON vs HTML — expectsJson()

`Handler.rs` decides whether to render HTML or pass through JSON, mirroring Laravel's `$request->expectsJson()`:

| Request | Result |
| --- | --- |
| Browser (`Accept: text/html`) | HTML error view |
| `Accept: application/json` | JSON |
| Axios / XHR (`X-Requested-With: XMLHttpRequest`) | JSON |
| `Content-Type: application/json` | JSON |

To force JSON for all `/api/*` routes, edit `expects_json()` in `src/app/Exceptions/Handler.rs`:

```rust
fn expects_json(request: &Request) -> bool {
    let is_api = request.uri().path().starts_with("/api/");
    is_api || wants_json || (is_ajax && accepts_any)
}
```

---

## Middleware

`src/middleware.rs` is the single place to manage middleware — analogous to Laravel's `Kernel.php`.

```rust
// Runs on every request (including session middleware)
pub fn global(state: Arc<AppState>, router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .layer(middleware::from_fn(LogRequest::handle))
        .layer(middleware::from_fn_with_state(Arc::clone(&state), session_middleware))
}

// Web (HTML) routes only
pub fn web(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
    // .layer(middleware::from_fn(my_app::authenticate))
}

// API routes only
pub fn api(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
}
```

Generate a new middleware skeleton:

```bash
willow-forge make:middleware RateLimiter
```

This creates `src/app/Http/Middleware/RateLimiter.rs` and registers `pub mod RateLimiter;` in `src/app/Http/Middleware/mod.rs`.

---

## Views

Views live under `resources/views/` as `.jinja.html` files.
The underlying engine is [MiniJinja](https://github.com/mitsuhiko/minijinja) (Jinja2 syntax).

### Rendering a view

```rust
use minijinja::context;
use my_app::{AppError, Context, view};

pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {
    Ok(view(&ctx, "welcome", context! {
        app_name => ctx.state.config.app_name.clone(),
    })?)
}
```

### View name lookup

Dot notation maps to nested folders:

| Name | File |
| --- | --- |
| `"welcome"` | `resources/views/welcome.jinja.html` |
| `"users.index"` | `resources/views/users/index.jinja.html` |
| `"layouts.app"` | `resources/views/layouts/app.jinja.html` |
| `"auth.login"` | `resources/views/auth/login.jinja.html` |

### Template syntax

```jinja
{% extends "layouts.app" %}

{% block title %}My Page{% endblock %}

{% block content %}
  {% if user %}
    <p>Hello, {{ user.name }}</p>
  {% else %}
    <p>Hello, guest</p>
  {% endif %}

  {% for item in items %}
    <li>{{ item.name }}</li>
  {% endfor %}
{% endblock %}
```

---

## Database

Willow Forge uses [sqlx](https://github.com/launchbainco/sqlx) for database access. **PostgreSQL** is the only supported database in v1.

### Database configuration (`.env`)

```env
DB_HOST=127.0.0.1
DB_PORT=5432
DB_DATABASE=willowforge
DB_USERNAME=postgres
DB_PASSWORD=postgres
```

### Migrations

```bash
willow-forge make:migration add_posts_table   # create a new migration pair
willow-forge migrate                          # run pending migrations
willow-forge migrate:rollback                 # undo last migration
willow-forge migrate:status                   # list applied / pending
willow-forge migrate:fresh                    # drop all + re-run
willow-forge migrate:reset                    # rollback all
```

Migration files live in `database/migrations/` as `.up.sql` / `.down.sql` pairs.

### Models

Models live in `src/app/Models/`. Derive `FromRow` for sqlx:

```rust
// src/app/Models/User.rs
use chrono::{DateTime, Utc};
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
```

Import the model in controllers:

```rust
use crate::app::Models::User::User;
```

### Querying

```rust
pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {
    let pool = &ctx.state.services.db;
    let users = sqlx::query_as::<_, User>(
        "SELECT id, name, email, password, created_at FROM users ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    Ok(Json(json!({ "data": users })))
}
```

---

## Redis and Cache

Willow Forge includes a Laravel-style Cache facade backed by a Redis Cluster.

### Redis configuration (`.env`)

```env
REDIS_CLUSTER_NODES=redis://127.0.0.1:7001,redis://127.0.0.1:7002,redis://127.0.0.1:7003
```

### Cache facade

```rust
use std::time::Duration;
use my_app::{Cache, Context, AppError};

// Get or compute-and-store
let users = Cache::remember(&ctx, "users.all", Duration::from_secs(300), || async {
    sqlx::query_as::<_, User>("SELECT * FROM users")
        .fetch_all(&ctx.state.services.db)
        .await
        .map_err(AppError::from)
}).await?;

// Simple get / put / delete
Cache::put(&ctx, "greeting", &"hello", Duration::from_secs(60)).await?;
let val: Option<String> = Cache::get(&ctx, "greeting").await?;
Cache::forget(&ctx, "greeting").await?;
```

| Method | Description |
| --- | --- |
| `Cache::get::<T>(&ctx, key)` | Retrieve value; `None` on cache miss or stale data |
| `Cache::put(&ctx, key, &val, ttl)` | Store with TTL |
| `Cache::put_forever(&ctx, key, &val)` | Store with no expiry |
| `Cache::remember(&ctx, key, ttl, \|\| async {...})` | Get or compute and store |
| `Cache::forget(&ctx, key)` | Delete a key |
| `Cache::has(&ctx, key)` | Check key existence |
| `Cache::increment` / `Cache::decrement` | Integer counters |

---

## Validation

Request structs live in `src/app/Http/Requests/`. Derive `Deserialize` and `Validate`:

```rust
// src/app/Http/Requests/StoreUserRequest.rs
use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct StoreUserRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}
```

Use `ValidatedJson<T>` for JSON API handlers (parses + validates automatically):

```rust
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<StoreUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    // req is already validated
}
```

Use `Form<T>` for HTML form handlers and call `.validate()` manually:

```rust
pub async fn store(
    ctx: Context,
    session: Session,
    Form(req): Form<LoginRequest>,
) -> impl IntoResponse {
    use validator::Validate;
    if let Err(errors) = req.validate() {
        let msg = errors.field_errors().values()
            .flat_map(|v| v.iter())
            .find_map(|e| e.message.clone())
            .unwrap_or_else(|| "Invalid input.".into());
        session.put("flash_error", msg.as_ref());
        return Redirect::to("/login").into_response();
    }
    // ...
}
```

---

## Auth

### Session-based auth — `make:auth`

`willow-forge make:auth` scaffolds HTML form-based authentication (login, register, logout).

```bash
willow-forge make:auth
```

What it generates:

| File | Description |
| --- | --- |
| `src/app/Http/Controllers/Auth/LoginController.rs` | `show` (form), `store` (login), `destroy` (logout) |
| `src/app/Http/Controllers/Auth/RegisterController.rs` | `show` (form), `store` (register) |
| `src/app/Http/Requests/login_request.rs` | Validated form request |
| `src/app/Http/Requests/register_request.rs` | Validated form request |
| `resources/views/auth/login.jinja.html` | Login form |
| `resources/views/auth/register.jinja.html` | Registration form |

Routes are injected automatically into `src/routes/web.rs`:

```rust
.route("/login",    get(LoginController::show).post(LoginController::store))
.route("/logout",   post(LoginController::destroy))
.route("/register", get(RegisterController::show).post(RegisterController::store))
```

Flash errors are shown on the form when validation or login fails.

#### Session API

```rust
use my_app::{Session, Auth};

// Put a value
session.put("key", value);

// Get and remove (flash pattern)
let msg: Option<String> = session.get("flash_error");
session.forget("flash_error");

// Auth helpers
Auth::login(&session, user_id);     // regenerates session, stores user id
Auth::logout(&session);             // invalidates session
Auth::check(&session) -> bool
Auth::id(&session) -> Option<i64>
```

Sessions are stored in Redis under `session:{id}` with a configurable TTL (default 7200 seconds).

#### Route protection

Two approaches are available.

**Option 1 — `AuthUser` extractor (single route)**

Add `AuthUser` as a parameter to any controller function. Unauthenticated requests are rejected automatically before the handler runs.

```rust
use my_app::AuthUser;

pub async fn dashboard(auth: AuthUser, ctx: Context) -> impl IntoResponse {
    // auth.id is the logged-in user's ID
    Json(json!({ "user_id": auth.id }))
}
```

**Option 2 — `authenticate` middleware (route group)**

Use `axum::middleware::from_fn(authenticate)` on a sub-router to protect multiple routes at once. Keep public routes (login, register) in a separate router to avoid redirect loops.

```rust
// src/routes/web.rs
use axum::{middleware, routing::{get, post}, Router};
use std::sync::Arc;
use my_app::{AppState, authenticate};

pub fn routes() -> Router<Arc<AppState>> {
    let protected = Router::new()
        .route("/dashboard", get(DashboardController::index))
        .route("/logout",    post(LoginController::destroy))
        .layer(middleware::from_fn(authenticate)); // unauthenticated → 302 /login

    let public = Router::new()
        .route("/login",    get(LoginController::show).post(LoginController::store))
        .route("/register", get(RegisterController::show).post(RegisterController::store));

    Router::new()
        .merge(protected)
        .merge(public)
}
```

> **Important:** never add `authenticate` to `/login` or `/register` — it causes an infinite redirect loop.

`AuthUser` and `authenticate` both return 302 → `/login` for browser requests and 401 JSON for `/api/*` or `Accept: application/json` requests.

### JWT-based auth — `make:auth --api`

`willow-forge make:auth --api` scaffolds stateless JWT authentication for REST API routes.

```bash
willow-forge make:auth --api
```

What it generates:

| File | Description |
| --- | --- |
| `src/app/Http/Controllers/Auth/LoginController.rs` | `store` (login → JWT token), `destroy` (logout → blacklist) |
| `src/app/Http/Controllers/Auth/RegisterController.rs` | `store` (register → JWT token) |

Routes are injected automatically into `src/routes/api.rs`:

```rust
.route("/api/auth/login",    post(LoginController::store))
.route("/api/auth/logout",   post(LoginController::destroy))
.route("/api/auth/register", post(RegisterController::store))
```

#### JWT API

```rust
use my_app::{Jwt, JwtUser};

// Issue a token
let token = Jwt::encode(user.id as i64)?;

// Blacklist a token on logout (stored in Redis until it expires)
let claims = Jwt::decode(&token)?;
let remaining = claims.exp.saturating_sub(Utc::now().timestamp() as usize) as u64;
Jwt::blacklist(&claims.jti, remaining, &ctx.state.services.redis).await?;

// Require authenticated API caller (401 if missing/invalid/blacklisted)
pub async fn profile(auth: JwtUser) -> impl IntoResponse {
    Json(json!({ "user_id": auth.id }))
}
```

Login response:

```json
{ "token": "eyJ...", "user": { "id": 1, "name": "Alice", "email": "alice@example.com" } }
```

Subsequent requests:

```http
Authorization: Bearer eyJ...
```

JWT tokens are signed with `JWT_SECRET` from `.env`. Default expiry is 3600 seconds (`JWT_EXPIRY`).

---

## CLI commands

| Command | Description |
| --- | --- |
| `willow-forge new <name>` | Scaffold a new application |
| `willow-forge make:controller <Name>` | Create `src/app/Http/Controllers/<Name>.rs` |
| `willow-forge make:request <Name>` | Create `src/app/Http/Requests/<Name>.rs` |
| `willow-forge make:model <Name>` | Create `src/app/Models/<Name>.rs` |
| `willow-forge make:middleware <Name>` | Create `src/app/Http/Middleware/<Name>.rs` |
| `willow-forge make:view <name>` | Create a view (dot notation: `users.index`) |
| `willow-forge make:migration <name>` | Create a timestamped migration pair |
| `willow-forge make:auth` | Scaffold session-based HTML auth (web routes) |
| `willow-forge make:auth --api` | Scaffold JWT-based API auth (api routes) |
| `willow-forge migrate` | Run pending migrations |
| `willow-forge migrate:rollback` | Roll back the last migration |
| `willow-forge migrate:status` | Show applied / pending migrations |
| `willow-forge migrate:fresh` | Drop all tables and re-run all migrations |
| `willow-forge migrate:reset` | Roll back all migrations |

All `make:*` commands automatically update the relevant `mod.rs` file, so there is no need to add module declarations manually.

> Use `cargo run` to start the application. There is no `willow-forge serve` command.

---
