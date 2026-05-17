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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, extract::Request as ExtractRequest, http::{Request, StatusCode, header}, middleware, routing::get, Router};
    use std::collections::HashMap;
    use tower::ServiceExt;

    async fn inject_empty(mut req: ExtractRequest, next: Next) -> Response {
        req.extensions_mut().insert(
            crate::session::Session::new_from_parts("sid".to_string(), HashMap::new(), true),
        );
        next.run(req).await
    }

    async fn inject_authed(mut req: ExtractRequest, next: Next) -> Response {
        let mut data = HashMap::new();
        data.insert("auth.user.id".to_string(), serde_json::json!(42i64));
        req.extensions_mut().insert(
            crate::session::Session::new_from_parts("sid".to_string(), data, false),
        );
        next.run(req).await
    }

    fn unauthed_app() -> Router {
        Router::new()
            .route("/dashboard", get(|| async { "ok" }))
            .route("/api/me", get(|| async { "ok" }))
            .layer(middleware::from_fn(handle))
            .layer(middleware::from_fn(inject_empty))
    }

    fn authed_app() -> Router {
        Router::new()
            .route("/dashboard", get(|| async { "ok" }))
            .route("/api/me", get(|| async { "ok" }))
            .layer(middleware::from_fn(handle))
            .layer(middleware::from_fn(inject_authed))
    }

    fn open_app() -> Router {
        Router::new().route("/health", get(|| async { "ok" }))
    }

    #[tokio::test]
    async fn am_01_unauthenticated_web_returns_302() {
        let req = Request::builder().uri("/dashboard").body(Body::empty()).unwrap();
        assert_eq!(unauthed_app().oneshot(req).await.unwrap().status(), StatusCode::FOUND);
    }

    #[tokio::test]
    async fn am_02_unauthenticated_api_returns_401() {
        let req = Request::builder().uri("/api/me").body(Body::empty()).unwrap();
        assert_eq!(unauthed_app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn am_03_authenticated_web_passes_through() {
        let req = Request::builder().uri("/dashboard").body(Body::empty()).unwrap();
        assert_eq!(authed_app().oneshot(req).await.unwrap().status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn am_04_authenticated_api_passes_through() {
        let req = Request::builder().uri("/api/me").body(Body::empty()).unwrap();
        assert_eq!(authed_app().oneshot(req).await.unwrap().status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn am_05_unauthenticated_redirect_location_is_slash_login() {
        let req = Request::builder().uri("/dashboard").body(Body::empty()).unwrap();
        let resp = unauthed_app().oneshot(req).await.unwrap();
        let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok());
        assert_eq!(loc, Some("/login"));
    }

    #[tokio::test]
    async fn am_06_accept_json_on_web_route_returns_401_not_302() {
        let req = Request::builder()
            .uri("/dashboard")
            .header(header::ACCEPT, "application/json")
            .body(Body::empty())
            .unwrap();
        assert_eq!(unauthed_app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn am_07_open_route_without_authenticate_always_200() {
        let req = Request::builder().uri("/health").body(Body::empty()).unwrap();
        assert_eq!(open_app().oneshot(req).await.unwrap().status(), StatusCode::OK);
    }
}
