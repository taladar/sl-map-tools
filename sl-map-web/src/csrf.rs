//! Cross-site request forgery defense.
//!
//! Reject state-changing requests whose `Origin` (or, failing that,
//! `Referer`) does not match the configured public base URL. The session
//! cookie uses `SameSite=Lax`, which on its own does not block same-site
//! sub-resource requests nor multipart form submissions from a third-party
//! origin, so cookie-authenticated routes need this second layer of defence.

use axum::extract::{Request, State};
use axum::http::{Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};

use crate::error::Error;
use crate::state::AppState;

/// Middleware that enforces same-origin policy on state-changing requests.
///
/// Safe methods (`GET`, `HEAD`, `OPTIONS`) are forwarded unchanged. Unsafe
/// methods must carry an `Origin` header matching `config.public_base_url`
/// (or, as a fallback for clients that omit `Origin`, a `Referer` whose
/// origin matches). Requests with neither header, or with a mismatched
/// value, are rejected with [`Error::Forbidden`].
pub async fn require_same_origin(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if is_safe_method(req.method()) {
        return next.run(req).await;
    }
    let expected = state.config.public_base_url.trim_end_matches('/');
    let headers = req.headers();
    if let Some(origin) = header_str(headers, &header::ORIGIN) {
        if origin.trim_end_matches('/') == expected {
            return next.run(req).await;
        }
        return Error::Forbidden(String::from("origin mismatch")).into_response();
    }
    if let Some(referer) = header_str(headers, &header::REFERER) {
        if origin_of(referer).is_some_and(|o| o == expected) {
            return next.run(req).await;
        }
        return Error::Forbidden(String::from("referer origin mismatch")).into_response();
    }
    Error::Forbidden(String::from("missing Origin and Referer headers")).into_response()
}

/// Methods exempt from the same-origin check (RFC 9110 "safe" methods).
const fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Read a header value as UTF-8, returning `None` if it is missing or not
/// valid UTF-8.
fn header_str<'a>(
    headers: &'a axum::http::HeaderMap,
    name: &header::HeaderName,
) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// Extract the origin (`scheme://host[:port]`) from a full URL by stopping
/// at the first `/` after `scheme://`. Returns `None` if the input does not
/// look like an absolute URL.
fn origin_of(url: &str) -> Option<&str> {
    let scheme_end = url.find("://")?;
    let after_scheme = scheme_end.checked_add(3)?;
    let rest = url.get(after_scheme..)?;
    let host_end = rest.find('/').unwrap_or(rest.len());
    let total_end = after_scheme.checked_add(host_end)?;
    url.get(..total_end)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::origin_of;

    #[test]
    fn origin_of_strips_path_and_query() {
        assert_eq!(
            origin_of("https://maps.example.org/library?x=1"),
            Some("https://maps.example.org"),
        );
    }

    #[test]
    fn origin_of_keeps_port() {
        assert_eq!(
            origin_of("http://localhost:3000/foo"),
            Some("http://localhost:3000"),
        );
    }

    #[test]
    fn origin_of_with_no_path() {
        assert_eq!(
            origin_of("https://maps.example.org"),
            Some("https://maps.example.org"),
        );
    }

    #[test]
    fn origin_of_rejects_non_absolute() {
        assert_eq!(origin_of("/library"), None);
    }
}
