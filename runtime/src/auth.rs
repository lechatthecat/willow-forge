use crate::session::Session;

const AUTH_ID_KEY: &str = "auth.user.id";

/// Auth facade for session-based authentication.
///
/// All methods operate on a `&Session` extractor parameter.
///
/// # Example
/// ```rust,ignore
/// Auth::login(&session, user.id);   // store user in session
/// Auth::check(&session);            // true if logged in
/// Auth::id(&session);               // Some(user_id) or None
/// Auth::logout(&session);           // clear session
/// ```
pub struct Auth;

impl Auth {
    /// Log a user in by storing their ID in the session.
    /// Regenerates the session ID to prevent session fixation attacks.
    pub fn login(session: &Session, user_id: i64) {
        session.regenerate();
        session.put(AUTH_ID_KEY, user_id);
    }

    /// Log out by invalidating the session (flushes data and issues a new session ID).
    pub fn logout(session: &Session) {
        session.invalidate();
    }

    /// Returns true if a user is currently authenticated.
    pub fn check(session: &Session) -> bool {
        session.get::<i64>(AUTH_ID_KEY).is_some()
    }

    /// Returns the authenticated user's ID, or None if not authenticated.
    pub fn id(session: &Session) -> Option<i64> {
        session.get::<i64>(AUTH_ID_KEY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use std::collections::HashMap;

    fn session() -> Session {
        Session::new_from_parts("test".to_string(), HashMap::new(), true)
    }

    #[test]
    fn check_returns_false_when_not_logged_in() {
        let s = session();
        assert!(!Auth::check(&s));
        assert_eq!(Auth::id(&s), None);
    }

    #[test]
    fn login_sets_user_id() {
        let s = session();
        Auth::login(&s, 42);
        assert!(Auth::check(&s));
        assert_eq!(Auth::id(&s), Some(42));
    }

    #[test]
    fn login_regenerates_session_id() {
        let s = session();
        let old_id = s.inner.lock().unwrap().session_id.clone();
        Auth::login(&s, 1);
        let new_id = s.inner.lock().unwrap().session_id.clone();
        assert_ne!(old_id, new_id);
    }

    #[test]
    fn logout_clears_user() {
        let s = session();
        Auth::login(&s, 99);
        Auth::logout(&s);
        assert!(!Auth::check(&s));
        assert_eq!(Auth::id(&s), None);
    }
}
