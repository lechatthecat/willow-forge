use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use std::sync::Arc;

use crate::app_state::AppState;

#[derive(Clone)]
pub struct Context {
    pub state: Arc<AppState>,
}

#[async_trait]
impl<S> FromRequestParts<S> for Context
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let app_state = Arc::<AppState>::from_ref(state);

        Ok(Context { state: app_state })
    }
}
