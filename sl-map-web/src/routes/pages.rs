//! Handlers for the public HTML pages: login form and set-password form.

use axum::http::header;
use axum::response::{Html, IntoResponse as _, Response};

/// HTML for the login page (embedded at compile time).
const LOGIN_HTML: &str = include_str!("../assets/login.html");
/// JS that drives the login form (kept external so the page can be served
/// under a strict `Content-Security-Policy` without `'unsafe-inline'`).
const LOGIN_JS: &str = include_str!("../assets/login.js");
/// HTML for the set-password page (embedded at compile time).
const SET_PASSWORD_HTML: &str = include_str!("../assets/set_password.html");
/// JS that drives the set-password form, external for the same reason as
/// [`LOGIN_JS`].
const SET_PASSWORD_JS: &str = include_str!("../assets/set_password.js");

/// `GET /login` — serve the embedded login form.
pub async fn login_page() -> Html<&'static str> {
    Html(LOGIN_HTML)
}

/// `GET /static/login.js` — serve the embedded login script.
pub async fn login_js() -> Response {
    js_response(LOGIN_JS)
}

/// `GET /set-password` — serve the embedded set-password form. The form's
/// JavaScript reads the `token` query parameter from `window.location` and
/// submits it together with the chosen password.
pub async fn set_password_page() -> Html<&'static str> {
    Html(SET_PASSWORD_HTML)
}

/// `GET /static/set_password.js` — serve the embedded set-password script.
pub async fn set_password_js() -> Response {
    js_response(SET_PASSWORD_JS)
}

/// Wrap a JS string in a `text/javascript` response.
fn js_response(body: &'static str) -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        body,
    )
        .into_response()
}
