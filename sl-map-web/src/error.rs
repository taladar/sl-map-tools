//! Application error type and HTTP mapping.

use axum::response::{IntoResponse, Response};
// the workspace clippy.toml enforces a project-wide rename for the http
// `StatusCode` type. `axum::http::StatusCode` is the same underlying type
// as `reqwest::StatusCode` so clippy demands the `ReqwestStatusCode` alias
// even though we are not pulling it in from reqwest.
use axum::http::StatusCode as ReqwestStatusCode;
use sl_map_apis::map_tiles::{MapError, MapTileCacheError};
use sl_map_apis::region::{CacheError as RegionCacheError, USBNotecardToGridRectangleError};
use sl_types::map::{LocationParseError, RegionName, USBNotecardLoadError};

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
    /// error in the GLW event cache (HTTP fetch / on-disk redb / JSON).
    #[error("GLW event cache error: {0}")]
    GlwEventCache(#[from] sl_glw::GlwEventCacheError),
    /// error parsing a TrueType font file.
    #[error("font parse error: {0}")]
    FontParse(#[from] ab_glyph::InvalidFont),
}

impl From<MapError> for Error {
    fn from(value: MapError) -> Self {
        Self::Map(Box::new(value))
    }
}

impl From<sl_map_apis::text::FontError> for Error {
    fn from(value: sl_map_apis::text::FontError) -> Self {
        match value {
            sl_map_apis::text::FontError::Read { source, .. } => Self::Io(source),
            sl_map_apis::text::FontError::Parse(invalid) => Self::FontParse(invalid),
        }
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

/// Returns `true` when the given sqlx error wraps a SQLite uniqueness
/// constraint violation. SQLite reports these as extended code 2067
/// (`SQLITE_CONSTRAINT_UNIQUE`) or, for an `INTEGER PRIMARY KEY` clash,
/// 1555 (`SQLITE_CONSTRAINT_PRIMARYKEY`). As with [`is_fk_violation`] we
/// also fall back to a substring check of the error message.
#[must_use]
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    let sqlx::Error::Database(db) = err else {
        return false;
    };
    if matches!(db.code().as_deref(), Some("2067" | "1555")) {
        return true;
    }
    db.message().contains("UNIQUE constraint failed")
}

/// the message shown when a region name given by the user does not exist on
/// the grid
///
/// Region names starting with `SSP` are usually the preview names of new
/// regions which are replaced by the name without that prefix once the region
/// is released, so old notecards often still refer to them.
fn unknown_region_message(region_name: &RegionName) -> String {
    let name = region_name.as_ref();
    match name.strip_prefix("SSP") {
        Some(released_name) if !released_name.is_empty() => format!(
            "The region \"{name}\" does not exist. Region names starting with \"SSP\" are usually \
             preview names that are replaced once the region is released, try \
             \"{released_name}\" instead."
        ),
        _ => format!(
            "The region \"{name}\" does not exist. Please check the spelling of the region name."
        ),
    }
}

/// the message shown when the maps website used to look up region positions
/// failed or returned something unexpected
fn region_lookup_unavailable_message(region_name: Option<&RegionName>) -> String {
    let what = region_name.map_or_else(
        || "the region".to_owned(),
        |region_name| format!("the region \"{region_name}\""),
    );
    format!(
        "Could not look up the position of {what} because the Second Life maps service did not \
         respond as expected. Please try again later."
    )
}

/// the HTTP status and user facing message for a region cache error,
/// lookup failures of the remote service are reported as a bad gateway with a
/// readable message, local cache failures as a generic internal error
fn region_cache_error_response(
    err: &RegionCacheError,
    region_name: Option<&RegionName>,
) -> (ReqwestStatusCode, String) {
    match err {
        RegionCacheError::GridCoordinatesHttpError(_)
        | RegionCacheError::RegionNameHttpError(_) => (
            ReqwestStatusCode::BAD_GATEWAY,
            region_lookup_unavailable_message(region_name),
        ),
        _ => (
            ReqwestStatusCode::INTERNAL_SERVER_ERROR,
            "internal error".to_owned(),
        ),
    }
}

/// the HTTP status and user facing message for an error resolving the grid
/// rectangle of a USB notecard
fn usb_notecard_to_grid_rectangle_error_response(
    err: &USBNotecardToGridRectangleError,
) -> (ReqwestStatusCode, String) {
    match err {
        USBNotecardToGridRectangleError::NoUSBNotecardWaypoints => (
            ReqwestStatusCode::BAD_REQUEST,
            "The notecard does not contain any waypoints.".to_owned(),
        ),
        USBNotecardToGridRectangleError::NoGridCoordinatesForRegion(region_name) => (
            ReqwestStatusCode::UNPROCESSABLE_ENTITY,
            unknown_region_message(region_name),
        ),
        USBNotecardToGridRectangleError::CacheError {
            region_name,
            source,
        } => region_cache_error_response(source, Some(region_name)),
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let region_error_response = match &self {
            Self::RegionCache(err) => Some(region_cache_error_response(err, None)),
            Self::USBNotecardToGridRectangle(err) => {
                Some(usb_notecard_to_grid_rectangle_error_response(err))
            }
            _ => None,
        };
        let status = match &self {
            Self::BadRequest(_)
            | Self::Multipart(_)
            | Self::USBNotecardParse(_)
            | Self::USBNotecardLoad(_)
            | Self::InvalidOrExpiredToken
            | Self::FontParse(_) => ReqwestStatusCode::BAD_REQUEST,
            Self::Unauthenticated | Self::InvalidCredentials => ReqwestStatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => ReqwestStatusCode::FORBIDDEN,
            Self::JobNotFound | Self::NotFound(_) => ReqwestStatusCode::NOT_FOUND,
            Self::JobNotFinished => ReqwestStatusCode::ACCEPTED,
            Self::RenderFailed(_) => ReqwestStatusCode::CONFLICT,
            Self::TooManyRequests { .. } => ReqwestStatusCode::TOO_MANY_REQUESTS,
            Self::USBNotecardToGridRectangle(_) | Self::RegionCache(_) => region_error_response
                .as_ref()
                .map_or(ReqwestStatusCode::INTERNAL_SERVER_ERROR, |(status, _)| {
                    *status
                }),
            Self::Map(_)
            | Self::MapTileCache(_)
            | Self::Image(_)
            | Self::Io(_)
            | Self::Json(_)
            | Self::Database
            | Self::PasswordHash(_)
            | Self::GlwEventCache(_) => ReqwestStatusCode::INTERNAL_SERVER_ERROR,
        };
        // The 500-class variants may carry filesystem paths (`Io`),
        // argon2 parameter detail (`PasswordHash`), decoder internals
        // (`Image`), or deserializer state (`Json`). Collapse them to a
        // generic body — the full Display is still recorded via the
        // `warn!` below so the operator gets the detail in logs.
        // Region lookup errors get a dedicated readable message (an unknown
        // region is the user's to fix, a failing maps service is not).
        let body_text = match (&self, region_error_response) {
            (_, Some((_, message))) => message,
            (
                Self::Io(_)
                | Self::Image(_)
                | Self::Json(_)
                | Self::Map(_)
                | Self::MapTileCache(_)
                | Self::RegionCache(_)
                | Self::USBNotecardToGridRectangle(_)
                | Self::PasswordHash(_),
                None,
            ) => "internal error".to_owned(),
            (_, None) => format!("{self}"),
        };
        let retry_after = match &self {
            Self::TooManyRequests { retry_after_secs } => Some(*retry_after_secs),
            _ => None,
        };
        // Several variants embed attacker-supplied bytes (request bodies,
        // notecard contents, multipart field names, …). Escape control
        // chars via `char::escape_debug` before logging so a payload like
        // `\n[ERROR] system breach` or an ANSI escape can't inject a fake
        // log line or hijack a tailing terminal. Demote 4xx (and the 202
        // JobNotFinished) to `debug!` to avoid a volume-amplification
        // DoS: only 5xx variants — which indicate genuine server bugs —
        // stay at `warn!`. Flip `RUST_LOG=sl_map_web=debug` to recover
        // client-error visibility for an investigation.
        let display = self.to_string();
        let safe: String = display.chars().flat_map(char::escape_debug).collect();
        if status.is_server_error() {
            tracing::warn!("request failed: {safe}");
        } else {
            tracing::debug!("request failed: {safe}");
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use sl_map_apis::region::RegionNameToGridCoordinatesError;

    #[test]
    fn unknown_ssp_region_suggests_released_name() -> Result<(), Box<dyn std::error::Error>> {
        let err = USBNotecardToGridRectangleError::NoGridCoordinatesForRegion(RegionName::try_new(
            "SSPOutlandish",
        )?);
        let (status, message) = usb_notecard_to_grid_rectangle_error_response(&err);
        assert_eq!(status, ReqwestStatusCode::UNPROCESSABLE_ENTITY);
        assert!(message.contains("\"SSPOutlandish\" does not exist"));
        assert!(message.contains("try \"Outlandish\" instead"));
        Ok(())
    }

    #[test]
    fn unknown_region_asks_to_check_spelling() -> Result<(), Box<dyn std::error::Error>> {
        let err = USBNotecardToGridRectangleError::NoGridCoordinatesForRegion(RegionName::try_new(
            "Thorkel",
        )?);
        let (status, message) = usb_notecard_to_grid_rectangle_error_response(&err);
        assert_eq!(status, ReqwestStatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            message,
            "The region \"Thorkel\" does not exist. Please check the spelling of the region name."
        );
        Ok(())
    }

    #[test]
    fn failing_maps_service_is_a_bad_gateway_naming_the_region()
    -> Result<(), Box<dyn std::error::Error>> {
        let err = USBNotecardToGridRectangleError::CacheError {
            region_name: RegionName::try_new("Ahern")?,
            source: RegionCacheError::GridCoordinatesHttpError(
                RegionNameToGridCoordinatesError::FailedToCloneRequest,
            ),
        };
        let (status, message) = usb_notecard_to_grid_rectangle_error_response(&err);
        assert_eq!(status, ReqwestStatusCode::BAD_GATEWAY);
        assert!(message.contains("position of the region \"Ahern\""));
        assert!(message.contains("try again later"));
        Ok(())
    }

    #[test]
    fn no_waypoints_is_a_bad_request() {
        let (status, _) = usb_notecard_to_grid_rectangle_error_response(
            &USBNotecardToGridRectangleError::NoUSBNotecardWaypoints,
        );
        assert_eq!(status, ReqwestStatusCode::BAD_REQUEST);
    }

    #[test]
    fn region_error_status_is_used_for_the_response() -> Result<(), Box<dyn std::error::Error>> {
        let response = Error::from(USBNotecardToGridRectangleError::NoGridCoordinatesForRegion(
            RegionName::try_new("Thorkel")?,
        ))
        .into_response();
        assert_eq!(response.status(), ReqwestStatusCode::UNPROCESSABLE_ENTITY);
        let response = Error::from(RegionCacheError::GridCoordinatesHttpError(
            RegionNameToGridCoordinatesError::FailedToCloneRequest,
        ))
        .into_response();
        assert_eq!(response.status(), ReqwestStatusCode::BAD_GATEWAY);
        Ok(())
    }
}
