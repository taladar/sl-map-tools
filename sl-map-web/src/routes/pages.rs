//! Handlers for the public HTML pages: login form and set-password form.

use axum::response::Html;

/// HTML for the login page (embedded at compile time).
const LOGIN_HTML: &str = include_str!("../assets/login.html");
/// HTML for the set-password page (embedded at compile time).
const SET_PASSWORD_HTML: &str = include_str!("../assets/set_password.html");

/// `GET /login` — serve the embedded login form.
pub async fn login_page() -> Html<&'static str> {
    Html(LOGIN_HTML)
}

/// `GET /set-password` — serve the embedded set-password form. The form's
/// JavaScript reads the `token` query parameter from `window.location` and
/// submits it together with the chosen password.
pub async fn set_password_page() -> Html<&'static str> {
    Html(SET_PASSWORD_HTML)
}
