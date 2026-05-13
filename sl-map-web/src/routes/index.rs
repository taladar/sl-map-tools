//! Handlers for the embedded static assets (HTML page + JS + CSS).

use axum::http::header;
use axum::response::{Html, IntoResponse as _, Response};

/// The HTML page is embedded at compile time so the binary is self-contained.
const INDEX_HTML: &str = include_str!("../assets/index.html");
/// The vanilla JS that drives the page.
const APP_JS: &str = include_str!("../assets/app.js");
/// The CSS shipped with the page.
const STYLE_CSS: &str = include_str!("../assets/style.css");

/// `GET /` — serve the embedded HTML page.
pub async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

/// `GET /static/app.js` — serve the embedded application JS.
pub async fn app_js() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        APP_JS,
    )
        .into_response()
}

/// `GET /static/style.css` — serve the embedded CSS.
pub async fn style_css() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLE_CSS,
    )
        .into_response()
}
