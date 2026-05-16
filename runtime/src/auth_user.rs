use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{auth::Auth, session::Session};

/// Axum extractor for the authenticated user. Fails with 401 (API) or 302 redirect (web)
/// if no user is logged in.
///
/// Controllers that carry `auth: AuthUser` as a parameter are automatically protected.
/// For route-group protection, use the `authenticate` middleware instead.
///
/// # Example
/// ```rust,ignore
/// pub async fn dashboard(auth: AuthUser, ctx: Context) -> impl IntoResponse {
///     // auth.id is guaranteed to be the logged-in user's ID
///     view("dashboard", context! { user_id: auth.id })
/// }
/// ```
pub struct AuthUser {
    pub id: i64,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Session middleware not installed")
                    .into_response()
            })?;

        if let Some(id) = Auth::id(&session) {
            return Ok(AuthUser { id });
        }

        Err(unauthenticated_response(parts))
    }
}

pub(crate) fn unauthenticated_response(parts: &Parts) -> Response {
    reject(parts.uri.path(), &parts.headers)
}

/// Shared rejection builder used by both `AuthUser` extractor and `authenticate` middleware.
pub(crate) fn reject(path: &str, headers: &axum::http::HeaderMap) -> Response {
    if expects_json(path, headers) {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response()
    } else {
        (StatusCode::FOUND, [(header::LOCATION, "/login")]).into_response()
    }
}

fn expects_json(path: &str, headers: &axum::http::HeaderMap) -> bool {
    if path.starts_with("/api/") {
        return true;
    }
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|a| a.contains("application/json"))
        .unwrap_or(false)
}
