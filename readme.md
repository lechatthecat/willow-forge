# Willow Framework

A Laravel-like web framework for Rust.

---

## Creating a new app

Install the CLI from the repo root:

```
cargo install --path .
```

Then scaffold a new application:

```
willow new my-app
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

Willow does DI via `Arc<AppState>` passed through the axum router state.

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
willow make:middleware Auth
```

This creates `app/Http/Middleware/Auth.rs` with a `handle()` stub and instructions for registering it in `bootstrap/middleware.rs`.

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

Willow uses [sqlx](https://github.com/launchbainco/sqlx) for database access. **PostgreSQL** is the only supported database in v1.

### Setup

```
DB_HOST=127.0.0.1
DB_PORT=5432
DB_DATABASE=willow
DB_USERNAME=postgres
DB_PASSWORD=postgres
```

### Migrations

```
willow migrate                          # run pending migrations
willow make:migration add_posts_table   # create a new migration pair
willow migrate:rollback                 # undo last migration
willow migrate:status                   # list applied / pending
willow migrate:fresh                    # drop all + re-run
willow migrate:reset                    # rollback all
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
| `willow new <name>` | Scaffold a new application |
| `willow make:controller <Name>` | Create `app/Http/Controllers/<Name>.rs` |
| `willow make:request <Name>` | Create `app/Http/Requests/<Name>.rs` |
| `willow make:model <Name>` | Create `app/Models/<Name>.rs` |
| `willow make:view <name>` | Create a view (dot notation supported) |
| `willow make:migration <name>` | Create a timestamped migration pair |
| `willow make:middleware <name>` | Create `app/Http/Middleware/<Name>.rs` |
| `willow migrate` | Run pending migrations |
| `willow migrate:rollback` | Roll back the last migration |
| `willow migrate:status` | Show applied / pending migrations |
| `willow migrate:fresh` | Drop all tables and re-run all migrations |
| `willow migrate:reset` | Roll back all migrations |

> Use `cargo run` to start the application. There is no `willow serve` command.

---

## v1 limitations

- No authentication or session handling
- No Blade-compatible template syntax (MiniJinja/Jinja2 syntax is used)
- No view components, slots, or custom directives
- No ActiveRecord-style ORM (raw sqlx queries)
- No queue or event system
- Error responses are always JSON (no HTML error pages in v1)
- `config/*.toml` files are generated but not loaded at runtime (env vars are used directly)
