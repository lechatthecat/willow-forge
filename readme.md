# Willow Forge

A laravel-inspired web framework for Rust.

---

## Creating a new app

Install the CLI from the repo root:

```
cargo install --path .
```

Then scaffold a new application:

```
willow-forge new my-app
cd my-app
cargo run
```

The server starts on `http://localhost:3000`.

Alternatively, run the CLI without installing:

```
cargo run -- new my-app
```

---

## Generated app layout

```
my-app/
├── app/
│   ├── errors.rs               ← AppError (unified error type)
│   ├── Http/
│   │   ├── Controllers/
│   │   │   ├── HomeController.rs
│   │   │   ├── UserController.rs
│   │   │   └── StatusController.rs
│   │   ├── Middleware/
│   │   │   └── LogRequest.rs
│   │   └── Requests/
│   │       └── StoreUserRequest.rs
│   ├── Models/
│   │   └── User.rs
│   └── Providers/
│       └── AppServiceProvider.rs
├── bootstrap/
│   ├── lib.rs              ← library root, bootstrap() lives here
│   ├── app_state.rs
│   ├── context.rs
│   ├── validated_json.rs
│   ├── view.rs
│   └── middleware.rs       ← global / api / web middleware groups
├── config/
│   ├── app.toml
│   └── database.toml
├── database/
│   └── migrations/
├── resources/
│   └── views/
│       ├── layouts/
│       │   └── app.jinja.html
│       └── welcome.jinja.html
├── routes/
│   ├── api.rs
│   └── web.rs
├── src/
│   └── main.rs
├── .env
└── Cargo.toml
```

---

## Routing

Routes live in `routes/web.rs` (HTML) and `routes/api.rs` (JSON).

Each file returns an `axum::Router<Arc<AppState>>`:

```rust
// routes/web.rs
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(home_controller::index))
}

// routes/api.rs
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/users", get(user_controller::index).post(user_controller::store))
        .route("/api/users/mock", get(user_controller::mock))
        .route("/api/status", get(status_controller::index))
}
```

Both routers are merged in `src/main.rs` with middleware applied:

```rust
let app = middleware::global(
    middleware::api(api::routes())
        .merge(middleware::web(web::routes())),
)
.with_state(app_state);
```

---

## AppState and Context (dependency injection)

Willow Forge does DI via `Arc<AppState>` passed through the axum router state.

### AppState

Defined in `bootstrap/app_state.rs`. Inner fields are plain values — no nested `Arc`:

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
    // ...
}
```

### Bootstrap

`bootstrap/lib.rs` wires everything together at startup:
1. Reads `.env`
2. Builds `Config` from environment variables
3. Initialises the view engine from `resources/views/`
4. Creates the database pool via `AppServiceProvider`
5. Returns `Arc<AppState>`

---

## Error handling

`app/errors.rs` defines `AppError`, which is re-exported as `use my_app::AppError`.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found")]             NotFound,                              // 404
    #[error("Unauthorized")]          Unauthorized,                          // 401
    #[error("Forbidden")]             Forbidden,                             // 403
    #[error("Validation failed")]     Validation(#[from] ValidationError),   // 422
    #[error("View render failed")]    View(#[from] ViewError),               // 500
    #[error("Database error")]        Database(#[from] sqlx::Error),         // 500
    #[error("Conflict: {0}")]         Conflict(String),                      // 409
    #[error("Internal server error")] Internal,                              // 500
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

### Error responses

| AppError variant | HTTP status | Body |
|-----------------|-------------|------|
| `NotFound` | 404 | `{"message":"Not found"}` |
| `Unauthorized` | 401 | `{"message":"Unauthorized"}` |
| `Forbidden` | 403 | `{"message":"Forbidden"}` |
| `Validation` | 422 | `{"message":"The given data was invalid.","errors":{...}}` |
| `Conflict(msg)` | 409 | `{"message":"<msg>"}` |
| `View` / `Database` / `Internal` | 500 | `{"message":"Internal server error"}` |

---

## Middleware

`bootstrap/middleware.rs` is the single place to manage middleware — analogous to Laravel's `Kernel.php`.

```rust
// Runs on every request
pub fn global(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.layer(middleware::from_fn(log_request::handle))
}

// API routes only
pub fn api(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
    // .layer(middleware::from_fn(auth::handle))
}

// Web (HTML) routes only
pub fn web(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
    // .layer(middleware::from_fn(csrf::handle))
}
```

Generate a new middleware skeleton:

```
willow-forge make:middleware Auth
```

This creates `app/Http/Middleware/Auth.rs` with a `handle()` stub and instructions for registering it in `bootstrap/middleware.rs`.

---

## Exception handling

### Custom error views

Error views live in `resources/views/errors/` as `.jinja.html` files.
`app/Exceptions/Handler.rs` intercepts every error response and renders the matching view when it exists.

| Template | Rendered when |
|----------|---------------|
| `resources/views/errors/404.jinja.html` | 404 Not Found |
| `resources/views/errors/500.jinja.html` | 500 Internal Server Error |
| `resources/views/errors/{code}.jinja.html` | Any other status code |

Variables available in every error view: `code`, `message`, `app_name`, `app_env`.

If no matching view exists the original JSON response (`AppError::IntoResponse`) is passed through unchanged.

### Route fallback

Undefined routes are caught by the `.fallback()` handler in `src/main.rs` and converted to `AppError::NotFound`:

```rust
// src/main.rs — generated automatically
async fn not_found() -> impl axum::response::IntoResponse {
    AppError::NotFound
}

// ...
.fallback(not_found)
```

### JSON vs HTML — expectsJson()

`app/Exceptions/Handler.rs` decides whether to render an HTML view or pass through JSON using `expects_json()`, which mirrors Laravel's `$request->expectsJson()`:

| Request | Result |
|---------|--------|
| Browser (`Accept: text/html`) | HTML error view |
| `curl` (no Accept header) | HTML error view |
| `curl -H "Accept: application/json"` | JSON |
| Axios / XHR (`X-Requested-With: XMLHttpRequest`) | JSON |
| `fetch()` with explicit JSON Accept | JSON |

To force JSON for all `/api/*` routes regardless of headers (common Laravel pattern via `shouldRenderJsonWhen`), edit `expects_json()` in `app/Exceptions/Handler.rs`:

```rust
fn expects_json(request: &Request) -> bool {
    let is_api = request.uri().path().starts_with("/api/");
    // ... existing header checks ...
    is_api || wants_json || (is_ajax && accepts_any)
}
```

### Customizing API error responses

**Option 1 — Return a custom response directly from a controller (full override):**

```rust
pub async fn show(ctx: Context) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Json(json!({
        "error": "user_not_found",
        "message": "No user with that ID exists."
    })))
}
```

**Option 2 — Edit `app/errors.rs` to change the JSON format for an existing error:**

```rust
AppError::NotFound => (
    StatusCode::NOT_FOUND,
    Json(json!({ "error": "not_found", "message": "Resource not found" })),
).into_response(),
```

**Option 3 — Add a new variant to `AppError` for a specific case:**

```rust
#[error("User not found: {0}")]
UserNotFound(i32),   // maps to 404 with the user's ID
```

### Customizing error handling

Open `app/Exceptions/Handler.rs` to add shared logic for all errors — logging, alerting, custom headers:

```rust
pub async fn render(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let status = response.status();

    if status.is_server_error() {
        tracing::error!("Server error: {}", status);  // add custom logic here
    }

    // render error view if available, otherwise pass through
    // ...
}
```

---

## Views

Views live under `resources/views/` as `.jinja.html` files.
The underlying engine is [MiniJinja](https://github.com/mitsuhiko/minijinja) (Jinja2 syntax).

### Rendering a view

```rust
use minijinja::context;
use my_app::{AppError, Context};
use my_app::view::view;

pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {
    Ok(view(&ctx, "welcome", context! {
        app_name => ctx.state.config.app_name.clone(),
    })?)
}
```

### View name lookup

Dot notation maps to nested folders:

| Name | File |
|------|------|
| `"welcome"` | `resources/views/welcome.jinja.html` |
| `"users.index"` | `resources/views/users/index.jinja.html` |
| `"layouts.app"` | `resources/views/layouts/app.jinja.html` |

### Template syntax

**Variable output** (HTML-escaped by default):
```
{{ app_name }}
```

**Conditionals:**
```
{% if user %}
  <p>Hello, {{ user.name }}</p>
{% else %}
  <p>Hello, guest</p>
{% endif %}
```

**Loops:**
```
{% for user in users %}
  <li>{{ user.name }}</li>
{% endfor %}
```

**Layout inheritance:**

In the child view:
```
{% extends "layouts.app" %}

{% block content %}
  <h1>Hello</h1>
{% endblock %}
```

In `layouts/app.jinja.html`:
```html
<body>
  {% block content %}{% endblock %}
</body>
```

**Includes:**
```
{% include "partials.nav" %}
```

---

## Database

Willow Forge uses [sqlx](https://github.com/launchbainco/sqlx) for database access. **PostgreSQL** is the only supported database in v1.

### Setup

```
DB_HOST=127.0.0.1
DB_PORT=5432
DB_DATABASE=willowforge
DB_USERNAME=postgres
DB_PASSWORD=postgres
```

### Migrations

```
willow-forge migrate                          # run pending migrations
willow-forge make:migration add_posts_table   # create a new migration pair
willow-forge migrate:rollback                 # undo last migration
willow-forge migrate:status                   # list applied / pending
willow-forge migrate:fresh                    # drop all + re-run
willow-forge migrate:reset                    # rollback all
```

Migration files live in `database/migrations/` as `.up.sql` / `.down.sql` pairs.

### Models

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
}
```

### Querying

```rust
pub async fn index(ctx: Context) -> Result<impl IntoResponse, AppError> {
    let pool = &ctx.state.services.db;

    let users = sqlx::query_as::<_, User>(
        "SELECT id, name, email, created_at FROM users ORDER BY id",
    )
    .fetch_all(pool)
    .await?;  // sqlx::Error → AppError::Database

    Ok(Json(json!({ "data": users })))
}
```

Unique constraint violations map to `AppError::Conflict` (409):

```rust
.map_err(|e| match e {
    sqlx::Error::Database(ref db) if db.constraint() == Some("users_email_key")
        => AppError::Conflict("Email already taken.".to_string()),
    other => AppError::Database(other),
})?
```

---

## Redis and Cache

Willow Forge includes a Laravel-style Cache facade backed by Redis.

Two connection pools are created automatically at startup:
- **`services.redis`** — raw Redis pool (DB 0). Use for direct Redis commands.
- **`services.cache_redis`** — cache-dedicated pool (DB 1). Use via the `Cache` facade.

### Setup

```
REDIS_HOST=127.0.0.1
REDIS_PORT=6379
REDIS_PASSWORD=
REDIS_DB=0          # raw Redis pool
REDIS_CACHE_DB=1    # Cache facade pool
```

### Cache facade

```rust
use std::time::Duration;
use myapp::{Cache, Context, AppError};

// Get or compute-and-store (most common pattern)
let users = Cache::remember(&ctx, "users.all", Duration::from_secs(300), || async {
    sqlx::query_as::<_, User>("SELECT * FROM users")
        .fetch_all(&ctx.state.services.db)
        .await
        .map_err(AppError::from)
}).await?;

// Simple get / put
Cache::put(&ctx, "greeting", &"hello", Duration::from_secs(60)).await?;
let val: Option<String> = Cache::get(&ctx, "greeting").await?;

// Delete
Cache::forget(&ctx, "greeting").await?;

// Counters
Cache::increment(&ctx, "page.views").await?;
```

| Method | Description |
|--------|-------------|
| `Cache::get::<T>(&ctx, key)` | Retrieve value; `None` on cache miss |
| `Cache::put(&ctx, key, &val, ttl)` | Store with TTL |
| `Cache::put_forever(&ctx, key, &val)` | Store with no expiry |
| `Cache::remember(&ctx, key, ttl, \|\| async {...})` | Get or compute and store |
| `Cache::remember_forever(&ctx, key, \|\| async {...})` | Remember without TTL |
| `Cache::forget(&ctx, key)` | Delete a key |
| `Cache::flush(&ctx)` | FLUSHDB (cache DB only) |
| `Cache::has(&ctx, key)` | Check key existence |
| `Cache::increment` / `Cache::decrement` | Integer counters |

### Direct Redis access

```rust
use redis::AsyncCommands;

let mut conn = ctx.state.services.redis.get().await?;
let _: () = conn.set_ex("raw:key", "value", 60u64).await?;
let val: Option<String> = conn.get("raw:key").await?;
```

---

## Validation

Request structs live in `app/Http/Requests/`. Derive `Deserialize` and `Validate`:

```rust
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

Use `ValidatedJson<T>` in handlers:

```rust
pub async fn store(
    ctx: Context,
    ValidatedJson(req): ValidatedJson<StoreUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    // req is fully validated here
}
```

| Situation | Status | Body |
|-----------|--------|------|
| Malformed JSON | 400 | `{"message":"Invalid JSON: ..."}` |
| Validation failure | 422 | `{"message":"The given data was invalid.","errors":{...}}` |

---

## CLI commands

| Command | Description |
|---------|-------------|
| `willow-forge new <name>` | Scaffold a new application |
| `willow-forge make:controller <Name>` | Create `app/Http/Controllers/<Name>.rs` |
| `willow-forge make:request <Name>` | Create `app/Http/Requests/<Name>.rs` |
| `willow-forge make:model <Name>` | Create `app/Models/<Name>.rs` |
| `willow-forge make:view <name>` | Create a view (dot notation supported) |
| `willow-forge make:migration <name>` | Create a timestamped migration pair |
| `willow-forge make:middleware <name>` | Create `app/Http/Middleware/<Name>.rs` |
| `willow-forge migrate` | Run pending migrations |
| `willow-forge migrate:rollback` | Roll back the last migration |
| `willow-forge migrate:status` | Show applied / pending migrations |
| `willow-forge migrate:fresh` | Drop all tables and re-run all migrations |
| `willow-forge migrate:reset` | Roll back all migrations |

> Use `cargo run` to start the application. There is no `willow-forge serve` command.

---

