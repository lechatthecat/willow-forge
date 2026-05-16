use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

pub(crate) struct SessionInner {
    pub session_id: String,
    /// Set when session ID is replaced (regenerate/invalidate); middleware DELs this key from Redis.
    pub old_session_id: Option<String>,
    pub data: HashMap<String, serde_json::Value>,
    pub dirty: bool,
    pub invalidated: bool,
    pub is_new: bool,
}

/// Redis-backed session, accessible from handlers as an Axum extractor.
///
/// The session middleware (`session_middleware`) must be registered globally in
/// `bootstrap/middleware.rs` — it loads and saves session data around each request.
///
/// # Example
/// ```rust,ignore
/// pub async fn login(session: Session, ...) -> impl IntoResponse {
///     Auth::login(&session, user.id);
///     Redirect::to("/dashboard")
/// }
/// ```
#[derive(Clone)]
pub struct Session {
    pub(crate) inner: Arc<Mutex<SessionInner>>,
}

impl Session {
    /// Retrieve a value from the session by key.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let inner = self.inner.lock().unwrap();
        inner
            .data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Store a value in the session. Marks the session dirty for Redis flush.
    pub fn put<T: Serialize>(&self, key: &str, value: T) {
        let Ok(v) = serde_json::to_value(value) else {
            return;
        };
        let mut inner = self.inner.lock().unwrap();
        inner.data.insert(key.to_string(), v);
        inner.dirty = true;
    }

    /// Remove a single key from the session.
    pub fn forget(&self, key: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.data.remove(key);
        inner.dirty = true;
    }

    /// Clear all session data.
    pub fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.data.clear();
        inner.dirty = true;
    }

    /// Generate a new session ID while keeping existing data (prevents session fixation).
    pub fn regenerate(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.old_session_id = Some(inner.session_id.clone());
        inner.session_id = Uuid::new_v4().to_string();
        inner.dirty = true;
    }

    /// Flush all data and generate a new session ID (used on logout).
    pub fn invalidate(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.old_session_id = Some(inner.session_id.clone());
        inner.session_id = Uuid::new_v4().to_string();
        inner.data.clear();
        inner.dirty = true;
        inner.invalidated = true;
    }

    pub(crate) fn new_from_parts(
        session_id: String,
        data: HashMap<String, serde_json::Value>,
        is_new: bool,
    ) -> Self {
        Session {
            inner: Arc::new(Mutex::new(SessionInner {
                session_id,
                old_session_id: None,
                data,
                dirty: false,
                invalidated: false,
                is_new,
            })),
        }
    }
}

impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Session>()
            .cloned()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Session middleware not installed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session() -> Session {
        Session::new_from_parts("test-id".to_string(), HashMap::new(), true)
    }

    #[test]
    fn put_and_get_roundtrip() {
        let s = make_session();
        s.put("name", "Alice");
        assert_eq!(s.get::<String>("name"), Some("Alice".to_string()));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let s = make_session();
        assert_eq!(s.get::<String>("missing"), None);
    }

    #[test]
    fn forget_removes_key() {
        let s = make_session();
        s.put("key", 42i64);
        s.forget("key");
        assert_eq!(s.get::<i64>("key"), None);
    }

    #[test]
    fn flush_clears_all() {
        let s = make_session();
        s.put("a", 1i64);
        s.put("b", 2i64);
        s.flush();
        assert_eq!(s.get::<i64>("a"), None);
        assert_eq!(s.get::<i64>("b"), None);
    }

    #[test]
    fn regenerate_changes_id_keeps_data() {
        let s = make_session();
        s.put("user", 42i64);
        let old_id = {
            s.inner.lock().unwrap().session_id.clone()
        };
        s.regenerate();
        let new_id = s.inner.lock().unwrap().session_id.clone();
        assert_ne!(new_id, old_id);
        assert_eq!(s.get::<i64>("user"), Some(42));
    }

    #[test]
    fn put_marks_dirty() {
        let s = make_session();
        assert!(!s.inner.lock().unwrap().dirty);
        s.put("x", 1i64);
        assert!(s.inner.lock().unwrap().dirty);
    }

    #[test]
    fn invalidate_clears_data_and_changes_id() {
        let s = make_session();
        s.put("user", 1i64);
        let old_id = s.inner.lock().unwrap().session_id.clone();
        s.invalidate();
        assert_eq!(s.get::<i64>("user"), None);
        let inner = s.inner.lock().unwrap();
        assert_ne!(inner.session_id, old_id);
        assert_eq!(inner.old_session_id, Some(old_id));
        assert!(inner.invalidated);
    }
}
