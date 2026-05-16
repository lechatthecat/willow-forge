use axum::{extract::Request, middleware::Next, response::Response};

use crate::{auth::Auth, auth_user::reject, session::Session};

/// Middleware that protects a route group from unauthenticated access.
///
/// Web routes get a 302 redirect to `/login`; API routes (`/api/*` or `Accept: application/json`)
/// get a 401 JSON response.
///
/// Register in `bootstrap/middleware.rs`:
/// ```rust,ignore
/// // Protect all web routes:
/// pub fn web(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
///     router.layer(axum::middleware::from_fn(my_app::authenticate))
/// }
///
/// // Or protect a specific route group inline:
/// Router::new()
///     .route("/dashboard", get(dashboard))
///     .layer(axum::middleware::from_fn(my_app::authenticate))
/// ```
pub async fn handle(session: Session, request: Request, next: Next) -> Response {
    if !Auth::check(&session) {
        let path = request.uri().path().to_owned();
        let headers = request.headers().clone();
        return reject(&path, &headers);
    }
    next.run(request).await
}
