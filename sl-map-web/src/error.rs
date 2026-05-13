//! Application error type and HTTP mapping.

use axum::response::{IntoResponse, Response};
// the workspace clippy.toml enforces a project-wide rename for the http
// `StatusCode` type. `axum::http::StatusCode` is the same underlying type
// as `reqwest::StatusCode` so clippy demands the `ReqwestStatusCode` alias
// even though we are not pulling it in from reqwest.
use axum::http::StatusCode as ReqwestStatusCode;
use sl_map_apis::map_tiles::{MapError, MapTileCacheError};
use sl_map_apis::region::{CacheError as RegionCacheError, USBNotecardToGridRectangleError};
use sl_types::map::{LocationParseError, USBNotecardLoadError};

/// Top-level error for the web service. Wraps the various library and IO
/// errors so that handlers can use `?` and the central `IntoResponse` impl
/// turns them into HTTP responses.
///
/// The bigger inner errors (`MapError`, `MapTileCacheError`,
/// `USBNotecardToGridRectangleError`) are boxed so the discriminant stays
/// small and `Result<T, Error>` doesn't blow up in size.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// the request body could not be parsed.
    #[error("invalid request: {0}")]
    BadRequest(String),
    /// the requested job id is unknown or has been evicted.
    #[error("job not found")]
    JobNotFound,
    /// the request asked for a part of a job that does not exist (e.g.
    /// `image-without-route` when no without-route image was saved).
    #[error("requested resource not found: {0}")]
    NotFound(String),
    /// the job is still running; the requested artifact is not ready yet.
    #[error("job not finished yet")]
    JobNotFinished,
    /// the underlying render job failed with the given message.
    #[error("render failed: {0}")]
    RenderFailed(String),
    /// multipart upload parsing failed.
    #[error("multipart error: {0}")]
    Multipart(#[from] axum::extract::multipart::MultipartError),
    /// a USB notecard line could not be parsed.
    #[error("could not parse USB notecard: {0}")]
    USBNotecardParse(#[from] LocationParseError),
    /// loading a USB notecard from disk failed (only used by the loader
    /// helper).
    #[error("could not load USB notecard: {0}")]
    USBNotecardLoad(#[from] USBNotecardLoadError),
    /// the region name to grid coordinates cache failed.
    #[error("region cache error: {0}")]
    RegionCache(Box<RegionCacheError>),
    /// converting a USB notecard to a grid rectangle failed.
    #[error("USB notecard rectangle resolution error: {0}")]
    USBNotecardToGridRectangle(Box<USBNotecardToGridRectangleError>),
    /// the map renderer returned an error.
    #[error("map render error: {0}")]
    Map(Box<MapError>),
    /// the map tile cache returned an error.
    #[error("map tile cache error: {0}")]
    MapTileCache(Box<MapTileCacheError>),
    /// image encoding failed.
    #[error("image encoding error: {0}")]
    Image(Box<image::ImageError>),
    /// generic I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// the request lacks a valid session cookie or bearer token.
    #[error("unauthenticated")]
    Unauthenticated,
    /// the authenticated user is not permitted to perform this action.
    #[error("forbidden: {0}")]
    Forbidden(String),
    /// the credentials presented at login do not match a known user.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// the set-password / reset token is missing, malformed, used, or
    /// expired.
    #[error("invalid or expired token")]
    InvalidOrExpiredToken,
    /// a database operation failed. Detail is logged via `tracing` to avoid
    /// leaking internals over HTTP.
    #[error("database error")]
    Database,
    /// argon2 hashing or verification failed.
    #[error("password hash error: {0}")]
    PasswordHash(String),
    /// the caller exhausted their per-user token bucket for a
    /// rate-limited create endpoint. The wrapped value is the number of
    /// seconds the client should wait before retrying; it is also
    /// emitted as a `Retry-After` HTTP header.
    #[error("rate limit exceeded")]
    TooManyRequests {
        /// recommended wait in whole seconds.
        retry_after_secs: u64,
    },
}

impl From<MapError> for Error {
    fn from(value: MapError) -> Self {
        Self::Map(Box::new(value))
    }
}

impl From<MapTileCacheError> for Error {
    fn from(value: MapTileCacheError) -> Self {
        Self::MapTileCache(Box::new(value))
    }
}

impl From<RegionCacheError> for Error {
    fn from(value: RegionCacheError) -> Self {
        Self::RegionCache(Box::new(value))
    }
}

impl From<USBNotecardToGridRectangleError> for Error {
    fn from(value: USBNotecardToGridRectangleError) -> Self {
        Self::USBNotecardToGridRectangle(Box::new(value))
    }
}

impl From<image::ImageError> for Error {
    fn from(value: image::ImageError) -> Self {
        Self::Image(Box::new(value))
    }
}

/// Returns `true` when the given sqlx error wraps a SQLite foreign-key
/// constraint violation. SQLite reports these as either extended code 787
/// (`SQLITE_CONSTRAINT_FOREIGNKEY`) or, for deferred / trigger-mediated
/// failures, 1811 (`SQLITE_CONSTRAINT_TRIGGER`). To be robust across both
/// we also fall back to a substring check of the error message.
#[must_use]
pub fn is_fk_violation(err: &sqlx::Error) -> bool {
    let sqlx::Error::Database(db) = err else {
        return false;
    };
    if matches!(db.code().as_deref(), Some("787" | "1811")) {
        return true;
    }
    db.message().contains("FOREIGN KEY")
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_)
            | Self::Multipart(_)
            | Self::USBNotecardParse(_)
            | Self::USBNotecardLoad(_)
            | Self::InvalidOrExpiredToken => ReqwestStatusCode::BAD_REQUEST,
            Self::Unauthenticated | Self::InvalidCredentials => ReqwestStatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => ReqwestStatusCode::FORBIDDEN,
            Self::JobNotFound | Self::NotFound(_) => ReqwestStatusCode::NOT_FOUND,
            Self::JobNotFinished => ReqwestStatusCode::ACCEPTED,
            Self::RenderFailed(_) => ReqwestStatusCode::CONFLICT,
            Self::TooManyRequests { .. } => ReqwestStatusCode::TOO_MANY_REQUESTS,
            Self::USBNotecardToGridRectangle(_)
            | Self::RegionCache(_)
            | Self::Map(_)
            | Self::MapTileCache(_)
            | Self::Image(_)
            | Self::Io(_)
            | Self::Json(_)
            | Self::Database
            | Self::PasswordHash(_) => ReqwestStatusCode::INTERNAL_SERVER_ERROR,
        };
        // The 500-class variants may carry filesystem paths (`Io`),
        // argon2 parameter detail (`PasswordHash`), decoder internals
        // (`Image`), or deserializer state (`Json`). Collapse them to a
        // generic body — the full Display is still recorded via the
        // `warn!` below so the operator gets the detail in logs.
        let body_text = match &self {
            Self::Io(_)
            | Self::Image(_)
            | Self::Json(_)
            | Self::Map(_)
            | Self::MapTileCache(_)
            | Self::RegionCache(_)
            | Self::USBNotecardToGridRectangle(_)
            | Self::PasswordHash(_) => "internal error".to_owned(),
            _ => format!("{self}"),
        };
        let retry_after = match &self {
            Self::TooManyRequests { retry_after_secs } => Some(*retry_after_secs),
            _ => None,
        };
        tracing::warn!("request failed: {self}");
        let body = serde_json::json!({ "error": body_text }).to_string();
        let mut response = (status, body).into_response();
        if let Ok(value) = axum::http::HeaderValue::from_str("application/json; charset=utf-8") {
            drop(
                response
                    .headers_mut()
                    .insert(axum::http::header::CONTENT_TYPE, value),
            );
        }
        if let Some(secs) = retry_after
            && let Ok(value) = axum::http::HeaderValue::from_str(&secs.to_string())
        {
            drop(
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value),
            );
        }
        response
    }
}
