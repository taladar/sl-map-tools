//! HTML page handlers for the library, groups, and invitations UIs. All
//! pages are static HTML embedded at compile time; the per-page JS fetches
//! data from the JSON endpoints.

use axum::http::header;
use axum::response::{Html, IntoResponse as _, Response};

/// HTML for `/library`.
const LIBRARY_HTML: &str = include_str!("../assets/library.html");
/// JS that drives the `/library` page.
const LIBRARY_JS: &str = include_str!("../assets/library.js");
/// HTML for `/groups` and `/groups/{id}`.
const GROUPS_HTML: &str = include_str!("../assets/groups.html");
/// JS that drives the groups page.
const GROUPS_JS: &str = include_str!("../assets/groups.js");
/// HTML for `/invitations`.
const INVITATIONS_HTML: &str = include_str!("../assets/invitations.html");
/// JS that drives the invitations page.
const INVITATIONS_JS: &str = include_str!("../assets/invitations.js");
/// HTML for `/profile` and `/profile/{id}`.
const PROFILE_HTML: &str = include_str!("../assets/profile.html");
/// JS that drives the profile page.
const PROFILE_JS: &str = include_str!("../assets/profile.js");
/// JS that provides in-page replacements for `confirm`/`prompt`/`alert`,
/// shared by the library, groups, and invitations pages.
const MODAL_JS: &str = include_str!("../assets/modal.js");

/// `GET /library` — serve the library UI.
pub async fn library() -> Html<&'static str> {
    Html(LIBRARY_HTML)
}

/// `GET /groups` and `/groups/{id}` — serve the groups UI. The same HTML
/// handles both list and detail views, switched client-side based on the
/// URL.
pub async fn groups() -> Html<&'static str> {
    Html(GROUPS_HTML)
}

/// `GET /invitations` — serve the invitations UI.
pub async fn invitations() -> Html<&'static str> {
    Html(INVITATIONS_HTML)
}

/// `GET /profile` and `/profile/{id}` — serve the profile UI. The page
/// reads the user id (or the current user, on the no-arg route) from
/// the URL and fetches the JSON via `/api/users/{id}`.
pub async fn profile() -> Html<&'static str> {
    Html(PROFILE_HTML)
}

/// `GET /static/library.js`.
pub async fn library_js() -> Response {
    js_response(LIBRARY_JS)
}

/// `GET /static/groups.js`.
pub async fn groups_js() -> Response {
    js_response(GROUPS_JS)
}

/// `GET /static/invitations.js`.
pub async fn invitations_js() -> Response {
    js_response(INVITATIONS_JS)
}

/// `GET /static/profile.js`.
pub async fn profile_js() -> Response {
    js_response(PROFILE_JS)
}

/// `GET /static/modal.js`.
pub async fn modal_js() -> Response {
    js_response(MODAL_JS)
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
