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

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_)
            | Self::Multipart(_)
            | Self::USBNotecardParse(_)
            | Self::USBNotecardLoad(_) => ReqwestStatusCode::BAD_REQUEST,
            Self::JobNotFound | Self::NotFound(_) => ReqwestStatusCode::NOT_FOUND,
            Self::JobNotFinished => ReqwestStatusCode::ACCEPTED,
            Self::RenderFailed(_) => ReqwestStatusCode::CONFLICT,
            Self::USBNotecardToGridRectangle(_)
            | Self::RegionCache(_)
            | Self::Map(_)
            | Self::MapTileCache(_)
            | Self::Image(_)
            | Self::Io(_)
            | Self::Json(_) => ReqwestStatusCode::INTERNAL_SERVER_ERROR,
        };
        tracing::warn!("request failed: {self}");
        (status, format!("{self}")).into_response()
    }
}
