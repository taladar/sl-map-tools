//! Error types for the `sl-glw` crate
//!
//! Three layered error enums plus an umbrella [`Error`] that wraps all of
//! them. Layout mirrors `sl_map_apis::region` so the crates compose well.

/// Errors that can occur while fetching a GLW event over HTTP and
/// decoding its JSON body.
#[expect(
    clippy::module_name_repetitions,
    reason = "the error types are the primary public surface of this module"
)]
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// reqwest-level transport error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// the cache layer was unable to clone the request for revalidation
    #[error("failed to clone request for creation of cache policy")]
    FailedToCloneRequest,
    /// the GLW server returned a non-success, non-404 status
    #[error("HTTP {status} from GLW server at {url}: {body}")]
    BadStatus {
        /// HTTP status code returned by the server
        status: reqwest::StatusCode,
        /// the URL that produced the error
        url: String,
        /// the response body (truncated by the caller if needed)
        body: String,
    },
    /// the configured base URL was not parseable as a [`url::Url`]
    #[error("invalid GLW base URL: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),
    /// the response body could not be parsed as JSON
    #[error("JSON decode error: {0}")]
    Json(#[from] serde_json::Error),
    /// the parsed JSON did not satisfy the GLW schema
    #[error("invalid event data: {0}")]
    Parse(#[from] ParseError),
}

/// Errors that arise from validating GLW JSON values against their
/// documented domains (e.g. wind direction must be 0..=359).
#[expect(
    clippy::module_name_repetitions,
    reason = "the error types are the primary public surface of this module"
)]
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// a numeric field fell outside its allowed range
    #[error("field {field} out of range: got {value}, expected {allowed}")]
    OutOfRange {
        /// the JSON path of the offending field (e.g. `"base.wind.dir"`)
        field: &'static str,
        /// the value rendered as a string (works for ints and floats)
        value: String,
        /// human-readable description of the allowed domain
        allowed: &'static str,
    },
    /// a required field was missing from the JSON document
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    /// a `areas` / `circles` key did not match the expected `area<N>` /
    /// `circle<N>` pattern. Currently informational only — we do not
    /// reject the key on the basis of name.
    #[error("invalid keyed-object name: {0}")]
    InvalidKey(String),
}

/// Errors that arise from the three-tier cache around GLW events.
#[expect(
    clippy::module_name_repetitions,
    reason = "the error types are the primary public surface of this module"
)]
#[derive(Debug, thiserror::Error)]
pub enum GlwEventCacheError {
    /// error decoding the JSON serialised cache policy
    #[error("error decoding the JSON serialized CachePolicy: {0}")]
    CachePolicyJsonDecodeError(#[from] serde_json::Error),
    /// the underlying redb database returned an error opening the file
    #[error("redb database error: {0}")]
    DatabaseError(#[from] redb::DatabaseError),
    /// the underlying redb transaction returned an error
    #[error("redb transaction error: {0}")]
    TransactionError(#[from] redb::TransactionError),
    /// the underlying redb table returned an error
    #[error("redb table error: {0}")]
    TableError(#[from] redb::TableError),
    /// the underlying redb storage returned an error
    #[error("redb storage error: {0}")]
    StorageError(#[from] redb::StorageError),
    /// the underlying redb commit returned an error
    #[error("redb commit error: {0}")]
    CommitError(#[from] redb::CommitError),
    /// the upstream HTTP fetch failed
    #[error("error looking up GLW event via HTTP: {0}")]
    FetchError(#[from] FetchError),
    /// system time error while computing cache freshness
    #[error("error handling system time for cache age calculations: {0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),
}

/// Errors that arise while rendering GLW data onto a map.
#[expect(
    clippy::module_name_repetitions,
    reason = "the error types are the primary public surface of this module"
)]
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// the bundled font could not be loaded
    #[error("failed to load bundled font for labels: {0}")]
    FontLoad(String),
}

/// Umbrella error type that wraps every error this crate can produce.
///
/// Useful when a caller needs a single `Result` type across fetch, parse,
/// cache and render boundaries.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// an HTTP fetch or response-decoding error
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// a JSON validation error
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// a cache-layer error
    #[error(transparent)]
    Cache(#[from] GlwEventCacheError),
    /// a render-layer error
    #[error(transparent)]
    Render(#[from] RenderError),
}
