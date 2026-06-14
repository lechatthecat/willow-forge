# Willow Forge  EFramework Specification

## Goal

Build a Rust web framework named **Willow Forge** that provides a **Laravel-like developer experience** while staying idiomatic and safe in Rust.

Key product goal: **"Looks like Laravel, runs like Rust"**.

- Laravel-like directory structure and command UX
- Rust-safe dependency injection via **AppState + Context extractor**
- Validation via `ValidatedJson<T>` (JSON API) and `Form<T>` (HTML forms)
- Session-based web auth and JWT-based API auth
- CLI similar to Artisan (`willow-forge new`, `make:*`, `migrate`)

## Non-goals (v1)

- No runtime reflection-based DI container
- No ActiveRecord-style full ORM (Eloquent). Use sqlx directly.
- No remember-me tokens (persistent cookie = GDPR consent burden; tracked as future issue)
- No email verification (tracked as future issue)
- No password reset (tracked as future issue)

---

## Tech stack

- HTTP: `axum 0.8` + `tower`
- Async runtime: `tokio`
- Serialization: `serde`, `serde_json`
- Validation: `validator`
- Config/env: `toml` + `dotenvy`
- CLI: `clap`
- Database: `sqlx` (PostgreSQL only in v1)
- Cache/sessions: Redis Cluster (`redis` crate, cluster-async feature)
- Templates: `minijinja` (Jinja2 syntax)
- Auth: `argon2` for password hashing, `jsonwebtoken` for JWT

---

## Repository structure

Two crates in one workspace:

```text
willow/
├── src/                        ↁEwillow-forge CLI binary
━E  ├── main.rs
━E  ├── commands/
━E  ━E  ├── make.rs
━E  ━E  ├── migrate.rs
━E  ━E  └── new.rs
━E  └── templates/
━E      └── app_files.rs        ↁEall generated file content
└── runtime/                    ↁEwillow-forge-runtime library
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── app_error.rs
        ├── app_state.rs
        ├── auth.rs
        ├── auth_user.rs
        ├── authenticate.rs
        ├── cache.rs
        ├── context.rs
        ├── hash.rs
        ├── jwt.rs
        ├── jwt_user.rs
        ├── session.rs
        ├── session_middleware.rs
        ├── validated_json.rs
        └── view.rs
```

---

## Generated app layout

All Rust source files live under `src/`. **No `#[path]` attributes are used anywhere.**
The standard Rust module system (`mod.rs` files) is used throughout.

```text
my-app/
├── src/
━E  ├── main.rs
━E  ├── middleware.rs
━E  ├── app/
━E  ━E  ├── mod.rs
━E  ━E  ├── http/
━E  ━E  ━E  ├── mod.rs
━E  ━E  ━E  ├── controllers/
━E  ━E  ━E  ━E  ├── mod.rs
━E  ━E  ━E  ━E  ├── home_controller.rs
━E  ━E  ━E  ━E  ├── user_controller.rs
━E  ━E  ━E  ━E  └── status_controller.rs
━E  ━E  ━E  ├── Middleware/
━E  ━E  ━E  ━E  ├── mod.rs
━E  ━E  ━E  ━E  └── log_request.rs
━E  ━E  ━E  └── Requests/
━E  ━E  ━E      ├── mod.rs
━E  ━E  ━E      └── store_user_request.rs
━E  ━E  ├── models/
━E  ━E  ━E  ├── mod.rs
━E  ━E  ━E  └── User.rs
━E  ━E  └── exceptions/
━E  ━E      ├── mod.rs
━E  ━E      └── Handler.rs
━E  ├── routes/
━E  ━E  ├── mod.rs
━E  ━E  ├── web.rs
━E  ━E  └── api.rs
━E  ├── lib.rs                  ↁElibrary crate root; re-exports runtime symbols
━E  ├── app_service_provider.rs ↁEDB pool + Redis cluster construction
━E  ├── config/
━E  ━E  ├── app.toml
━E  ━E  ├── auth.toml
━E  ━E  ├── cache.toml
━E  ━E  ├── database.toml
━E  ━E  ├── jwt.toml
━E  ━E  └── mail.toml
━E  ├── database/
━E  ━E  └── migrations/
━E  ├── resources/
━E  ━E  └── views/
━E  ━E      ├── layouts/app.jinja.html
━E  ━E      ├── errors/
━E  ━E      └── welcome.jinja.html
━E  └── docker/
━E     └── docker-compose.yml
├── .env
└── Cargo.toml
```

---

## Architecture

### Module system rules

- All Rust source files for the binary crate go under `src/`.
- Controllers are in `src/app/http/controllers/`.
- Models are in `src/app/models/`.
- Routes are in `src/routes/`.
- Middleware wiring is in `src/middleware.rs`.
- `#[path]` is **forbidden** everywhere. Use `mod.rs` files instead.
- All `make:*` commands automatically append `pub mod name;` to the relevant `mod.rs`.

### AppState and Context

`Arc<AppState>` is the dependency injection mechanism. It is stored in the axum router state and pulled out by the `Context` extractor:

```rust
pub struct AppState {
    pub config: Config,
    pub services: Services,  // contains PgPool and Arc<ClusterClient>
    pub views: ViewEngine,
}

// In any handler:
pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {
    let pool = &ctx.state.services.db;
    let redis = &ctx.state.services.redis;
}
```

### Bootstrap sequence (`src/lib.rs`)

1. Load `.env` via `dotenvy`
2. Load `src/config/*.toml`
3. Build `Config`, with environment variables overriding config file defaults
4. Initialise `ViewEngine` (MiniJinja) from `resources/views/`
5. Create `PgPool` and `Arc<ClusterClient>` via `app_service_provider`
6. Return `Arc<AppState>`

### Routing

Routes use `crate::app::http::controllers::*` imports  Eno `#[path]`:

```rust
// src/routes/web.rs
use crate::app::http::controllers::home_controller;
use axum::{routing::get, Router};
use std::sync::Arc;
use my_app::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(home_controller::index))
}
```

### Middleware (`src/middleware.rs`)

Three groups mirroring Laravel's `Kernel.php`:

```rust
pub fn global(state: Arc<AppState>, router: Router<Arc<AppState>>) -> Router<Arc<AppState>>
pub fn web(router: Router<Arc<AppState>>) -> Router<Arc<AppState>>
pub fn api(router: Router<Arc<AppState>>) -> Router<Arc<AppState>>
```

`global()` wires session middleware via `from_fn_with_state(state, session_middleware)`.

---

## Runtime library (`willow-forge-runtime`)

Re-exported from `src/lib.rs` so generated apps import from their own crate name:

```rust
use my_app::{AppError, AppState, Auth, AuthUser, Cache, Context, Hash,
             Jwt, JwtUser, Session, ValidatedJson, authenticate,
             session_middleware, view};
```

### AppError

```rust
pub enum AppError {
    NotFound,
    Unauthorized,
    Forbidden,
    Validation(ValidationError),
    Conflict(String),
    ServiceUnavailable,
    TooManyRequests,
    Http(u16, String),
    View(ViewError),
    Database(sqlx::Error),
    Redis(redis::RedisError),
    Internal,
}
```

`From` impls allow `?` to propagate `sqlx::Error`, `ViewError`, and `redis::RedisError` automatically.

### Hash

```rust
pub struct Hash;
impl Hash {
    pub fn make(password: &str) -> Result<String, AppError>  // argon2id PHC string
    pub fn check(password: &str, hash: &str) -> bool
}
```

### Session

Arc-wrapped session state stored in request extensions by `session_middleware`. Backed by Redis (`session:{id}`). TTL, cookie name, and secure flag come from `src/config/auth.toml`, with `SESSION_*` env overrides.

```rust
impl Session {
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>
    pub fn put<T: Serialize>(&self, key: &str, value: T)
    pub fn forget(&self, key: &str)
    pub fn flush(&self)
    pub fn regenerate(&self)
    pub fn invalidate(&self)
}
```

### Auth

```rust
pub struct Auth;
impl Auth {
    pub fn login(session: &Session, user_id: i64)  // regenerates session
    pub fn logout(session: &Session)               // invalidates session
    pub fn check(session: &Session) -> bool
    pub fn id(session: &Session) -> Option<i64>
}
```

### AuthUser

Axum extractor that rejects unauthenticated requests:

- Browser requests ↁE302 redirect to `/login`
- API/AJAX requests ↁE401 JSON `{"message":"Unauthenticated."}`

### Jwt

```rust
pub struct Claims { pub sub: i64, pub jti: String, pub exp: usize }

pub struct Jwt;
impl Jwt {
    pub fn encode(user_id: i64) -> Result<String, AppError>
    pub fn decode(token: &str) -> Result<Claims, AppError>
    pub async fn blacklist(jti: &str, remaining_secs: u64, redis: &Arc<ClusterClient>) -> Result<(), AppError>
    pub async fn is_blacklisted(jti: &str, redis: &Arc<ClusterClient>) -> Result<bool, AppError>
}
```

Tokens are signed with `src/config/jwt.toml` (`JWT_SECRET` and `JWT_EXPIRY` env overrides). Redis blacklist key: `jwt:blacklist:{jti}` with TTL = remaining expiry.

### JwtUser

Axum extractor for JWT-authenticated API routes. Reads `Authorization: Bearer {token}`, decodes it, checks the Redis blacklist. Returns 401 on any failure.

```rust
pub struct JwtUser { pub id: i64, pub jti: String }
```

### Cache

Laravel-style facade backed by Redis Cluster:

```rust
Cache::get::<T>(&ctx, key) -> Result<Option<T>, AppError>
Cache::put(&ctx, key, &val, ttl) -> Result<(), AppError>
Cache::remember(&ctx, key, ttl, || async { ... }) -> Result<T, AppError>
Cache::forget(&ctx, key) -> Result<(), AppError>
Cache::has(&ctx, key) -> Result<bool, AppError>
Cache::increment(&ctx, key) -> Result<i64, AppError>
Cache::decrement(&ctx, key) -> Result<i64, AppError>
```

On deserialisation failure (stale data), `Cache::get` evicts the key and returns `None`.

---

## CLI commands

| Command | Description |
| --- | --- |
| `willow-forge new <name>` | Scaffold a new application |
| `willow-forge make:controller <Name>` | Create controller + register in `mod.rs` |
| `willow-forge make:request <Name>` | Create request struct + register in `mod.rs` |
| `willow-forge make:model <Name>` | Create model + register in `mod.rs` |
| `willow-forge make:middleware <Name>` | Create middleware + register in `mod.rs` |
| `willow-forge make:view <name>` | Create view (dot notation) |
| `willow-forge make:migration <name>` | Create timestamped `.up.sql` / `.down.sql` pair |
| `willow-forge make:auth` | Scaffold session-based HTML auth (web routes) |
| `willow-forge make:auth --api` | Scaffold JWT-based API auth (api routes) |
| `willow-forge migrate` | Run pending migrations |
| `willow-forge migrate:rollback` | Rollback last migration |
| `willow-forge migrate:status` | Show applied / pending migrations |
| `willow-forge migrate:fresh` | Drop all + re-run |
| `willow-forge migrate:reset` | Rollback all |

### make:auth behaviour

Both variants:

- Create `src/app/http/controllers/Auth/` with `login_controller.rs` and `register_controller.rs`
- Create `src/app/http/controllers/Auth/mod.rs`
- Inject `pub mod auth;` into `src/app/http/controllers/mod.rs`
- Create users migration if none exists in `database/migrations/`
- Inject routes into the appropriate routes file (no manual `mod` declarations needed)

Session variant additionally creates:

- `src/app/http/requests/login_request.rs` and `register_request.rs`
- `resources/views/auth/login.jinja.html` and `register.jinja.html`
- Injects `pub mod login_request; pub mod register_request;` into `src/app/http/requests/mod.rs`

---

## Coding standards

- Rust 2024 edition
- English only in all generated files, templates, comments, and user-facing text
- No `#[path]` anywhere in generated code or the CLI itself
- No unnecessary comments  Eonly when the WHY is non-obvious
- `make:*` commands always update the relevant `mod.rs` automatically
- Controllers use `crate::app::models::name::Name` to reference models
- Session auth uses `Form<T>` + manual `req.validate()` call; validation errors go to `flash_error` session key
- JWT auth uses `ValidatedJson<T>`; validation errors return 422 JSON via `AppError`
