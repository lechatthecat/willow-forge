use axum::response::{Html, IntoResponse, Response};
use serde::Serialize;

use crate::context::Context;

/// Render a view template and return an HTML response.
///
/// ```rust,ignore
/// use minijinja::context;
/// return view(&ctx, "welcome", context! { title => "Home" });
/// ```
pub fn view<S>(ctx: &Context, name: &str, data: S) -> Result<HtmlView, ViewError>
where
    S: Serialize,
{
    let tmpl = ctx
        .state
        .views
        .get_template(name)
        .map_err(|e| ViewError::NotFound(name.to_string(), e.to_string()))?;

    let rendered = tmpl
        .render(data)
        .map_err(|e| ViewError::RenderError(name.to_string(), e.to_string()))?;

    Ok(HtmlView(rendered))
}

pub struct HtmlView(String);

impl IntoResponse for HtmlView {
    fn into_response(self) -> Response {
        Html(self.0).into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error("View '{0}' not found: {1}")]
    NotFound(String, String),

    #[error("Failed to render view '{0}': {1}")]
    RenderError(String, String),
}

impl IntoResponse for ViewError {
    fn into_response(self) -> Response {
        use axum::http::StatusCode;
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}
