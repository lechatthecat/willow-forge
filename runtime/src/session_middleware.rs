use std::{collections::HashMap, sync::Arc};

use axum::{extract::Request, middleware::Next, response::Response};
use redis::AsyncCommands;
use uuid::Uuid;

use crate::{app_state::AppState, session::Session};

const SESSION_KEY_PREFIX: &str = "session:";

/// Session middleware. Capture `Arc<AppState>` via a closure in `bootstrap/middleware.rs`:
///
/// ```rust,ignore
/// let sess = Arc::clone(&state);
/// router.layer(axum::middleware::from_fn(move |req, next| {
///     let s = Arc::clone(&sess);
///     async move { session_middleware(s, req, next).await }
/// }))
/// ```
pub async fn handle(state: Arc<AppState>, mut request: Request, next: Next) -> Response {
    if !state.config.session.enabled {
        return next.run(request).await;
    }

    let ttl = state.config.session.lifetime;
    let cookie_name = state.config.session.cookie.as_str();

    let cookie_value = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_cookie(s, cookie_name));

    let (session_id, data, is_new) = match cookie_value {
        Some(id) if !id.is_empty() => match load_from_redis(&state, &id).await {
            Some(data) => (id, data, false),
            None => (id, HashMap::new(), false),
        },
        _ => (Uuid::new_v4().to_string(), HashMap::new(), true),
    };

    let session = Session::new_from_parts(session_id, data, is_new);
    request.extensions_mut().insert(session.clone());

    let mut response = next.run(request).await;

    // Collect everything we need while holding the lock, then drop the lock before .await
    struct FlushPlan {
        old_id: Option<String>,
        new_id: String,
        data_json: String,
        invalidated: bool,
        needs_write: bool,
        needs_cookie: bool,
    }

    let plan = {
        let inner = session.inner.lock().unwrap();
        FlushPlan {
            old_id: inner.old_session_id.clone(),
            new_id: inner.session_id.clone(),
            data_json: serde_json::to_string(&inner.data).unwrap_or_else(|_| "{}".to_string()),
            invalidated: inner.invalidated,
            needs_write: inner.dirty || inner.invalidated,
            needs_cookie: inner.is_new || inner.old_session_id.is_some(),
        }
        // MutexGuard drops here — safe to .await below
    };

    if plan.needs_write {
        let redis = Arc::clone(&state.services.redis);

        if let Some(ref old_id) = plan.old_id {
            let key = format!("{}{}", SESSION_KEY_PREFIX, old_id);
            if let Ok(mut conn) = redis.get_async_connection().await {
                let _: Result<(), _> = conn.del(&key).await;
            }
        }

        if !plan.invalidated {
            let key = format!("{}{}", SESSION_KEY_PREFIX, plan.new_id);
            if let Ok(mut conn) = redis.get_async_connection().await {
                let _: Result<(), _> = conn.set_ex(&key, &plan.data_json, ttl).await;
            }
        }
    }

    if plan.needs_cookie {
        let secure = state.config.session.secure;

        let mut cookie_str = format!(
            "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
            cookie_name, plan.new_id, ttl
        );
        if secure {
            cookie_str.push_str("; Secure");
        }

        if let Ok(val) = cookie_str.parse() {
            response.headers_mut().insert("set-cookie", val);
        }
    }

    response
}

async fn load_from_redis(
    state: &Arc<AppState>,
    session_id: &str,
) -> Option<HashMap<String, serde_json::Value>> {
    let key = format!("{}{}", SESSION_KEY_PREFIX, session_id);
    let mut conn = state.services.redis.get_async_connection().await.ok()?;
    let raw: Option<String> = conn.get(&key).await.ok()?;
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

fn parse_cookie(header: &str, name: &str) -> Option<String> {
    header.split(';').find_map(|part| {
        let part = part.trim();
        let (k, v) = part.split_once('=')?;
        if k.trim() == name {
            Some(v.trim().to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::parse_cookie;

    #[test]
    fn parse_single_cookie() {
        assert_eq!(
            parse_cookie("willow_session=abc123", "willow_session"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn parse_multiple_cookies() {
        assert_eq!(
            parse_cookie("foo=bar; willow_session=xyz; other=val", "willow_session"),
            Some("xyz".to_string())
        );
    }

    #[test]
    fn parse_missing_cookie_returns_none() {
        assert_eq!(parse_cookie("foo=bar; baz=qux", "willow_session"), None);
    }
}
