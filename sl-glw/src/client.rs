//! HTTP client for the GLW `glwDataReq.php` endpoint.
//!
//! Two layers are provided:
//!
//! 1. [`GlwClient`] — a small struct holding a `reqwest::Client` and the
//!    base URL. Use this when you just need a one-off fetch with no
//!    persistent cache.
//! 2. Free functions [`fetch_event_by_id`] and [`fetch_event_by_key`] —
//!    take an already-built `reqwest::Client` plus an optional cached
//!    `(value, CachePolicy)` pair so they can participate in
//!    `http-cache-semantics` revalidation. The [`GlwEventCache`](crate::GlwEventCache)
//!    persistent cache layer calls these.
//!
//! The `glw127` segment of the default URL is the GLW protocol version
//! Lalia bumps periodically (we have observed `glw110`, `glw120`,
//! `glw122`, `glw127`). Override with [`GlwClient::with_glw_version`] or
//! supply a fully custom base URL.

use crate::error::FetchError;
use crate::types::{EventId, GlwEvent, GlwEventKey};

/// Default protocol-and-host segment of the GLW URL.
pub const DEFAULT_GLW_HOST: &str = "http://globalwind.net";

/// Default GLW protocol version segment in the path.
pub const DEFAULT_GLW_VERSION: &str = "glw127";

/// Filename segment of the event-data endpoint.
const GLW_DATA_REQ_PATH: &str = "glwDataReq.php";

/// Build the default base URL from [`DEFAULT_GLW_HOST`] and
/// [`DEFAULT_GLW_VERSION`].
///
/// # Errors
///
/// Returns [`FetchError::InvalidBaseUrl`] if the composed URL fails to
/// parse — unreachable for the default constants but propagated for
/// completeness.
pub fn default_base_url() -> Result<url::Url, FetchError> {
    let raw = format!("{DEFAULT_GLW_HOST}/{DEFAULT_GLW_VERSION}/");
    url::Url::parse(&raw).map_err(FetchError::from)
}

/// Build a base URL from a custom version segment on the default host.
///
/// # Errors
///
/// Returns [`FetchError::InvalidBaseUrl`] if the composed URL fails to
/// parse.
pub fn base_url_for_version(version: &str) -> Result<url::Url, FetchError> {
    let raw = format!("{DEFAULT_GLW_HOST}/{version}/");
    url::Url::parse(&raw).map_err(FetchError::from)
}

/// Convenience HTTP client for the GLW event endpoint.
///
/// Holds a single shared `reqwest::Client` and a base URL. For a
/// persistent on-disk cache layer on top of this, see
/// [`crate::GlwEventCache`].
#[expect(
    clippy::module_name_repetitions,
    reason = "GlwClient is the primary public type of this module"
)]
#[derive(Debug, Clone)]
pub struct GlwClient {
    /// Underlying reqwest HTTP client.
    http: reqwest::Client,
    /// Base URL the `glwDataReq.php` path is joined onto.
    base_url: url::Url,
}

impl GlwClient {
    /// Build a client that targets the workspace default GLW host and
    /// protocol version.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::InvalidBaseUrl`] if the default URL fails
    /// to parse — unreachable for the default constants but propagated
    /// for completeness.
    pub fn new() -> Result<Self, FetchError> {
        Self::with_base_url(default_base_url()?)
    }

    /// Build a client targeting a custom GLW base URL (must include the
    /// version segment as a path, e.g. `http://example.com/glw127/`).
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::InvalidBaseUrl`] only if the supplied URL
    /// is in an unusable shape; otherwise infallible.
    pub fn with_base_url(base_url: url::Url) -> Result<Self, FetchError> {
        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
        })
    }

    /// Build a client targeting [`DEFAULT_GLW_HOST`] with a non-default
    /// version segment.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::InvalidBaseUrl`] if the composed URL fails
    /// to parse.
    pub fn with_glw_version(version: &str) -> Result<Self, FetchError> {
        Self::with_base_url(base_url_for_version(version)?)
    }

    /// Build a client over an existing `reqwest::Client` (lets the
    /// caller reuse connection pools and shared middleware).
    #[must_use]
    pub const fn with_client(http: reqwest::Client, base_url: url::Url) -> Self {
        Self { http, base_url }
    }

    /// Borrow the underlying base URL.
    #[must_use]
    pub const fn base_url(&self) -> &url::Url {
        &self.base_url
    }

    /// Borrow the underlying `reqwest::Client`.
    #[must_use]
    pub const fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Fetch a GLW event by numeric id (`?id=…`).
    ///
    /// Returns `Ok(None)` for HTTP 404 or a JSON `{}` body, both
    /// interpreted as "no such event".
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] on any HTTP, parse or transport failure.
    #[tracing::instrument(skip(self))]
    pub async fn fetch_event_by_id(&self, id: EventId) -> Result<Option<GlwEvent>, FetchError> {
        let (event, _policy) = fetch_event_by_id(&self.http, &self.base_url, id, None).await?;
        Ok(event)
    }

    /// Fetch a GLW event by string event-key (`?key=…`).
    ///
    /// Returns `Ok(None)` for HTTP 404 or a JSON `{}` body.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] on any HTTP, parse or transport failure.
    #[tracing::instrument(skip(self))]
    pub async fn fetch_event_by_key(
        &self,
        key: &GlwEventKey,
    ) -> Result<Option<GlwEvent>, FetchError> {
        let (event, _policy) = fetch_event_by_key(&self.http, &self.base_url, key, None).await?;
        Ok(event)
    }
}

/// Cached value alongside its `http-cache-semantics` policy, used as
/// both the input revalidation hint and the output of the lower-level
/// fetch functions.
type CachedEvent = (Option<GlwEvent>, http_cache_semantics::CachePolicy);

/// Lower-level fetch by numeric event id. Participates in
/// `http-cache-semantics` revalidation when a `cached` pair is supplied.
///
/// # Errors
///
/// Returns [`FetchError`] on any HTTP, parse or transport failure.
#[tracing::instrument(skip(client, cached))]
pub async fn fetch_event_by_id(
    client: &reqwest::Client,
    base_url: &url::Url,
    id: EventId,
    cached: Option<CachedEvent>,
) -> Result<CachedEvent, FetchError> {
    let request = build_request(client, base_url, "id", &id.get().to_string())?;
    fetch_one(client, request, cached).await
}

/// Lower-level fetch by string event key. Participates in
/// `http-cache-semantics` revalidation when a `cached` pair is supplied.
///
/// # Errors
///
/// Returns [`FetchError`] on any HTTP, parse or transport failure.
#[tracing::instrument(skip(client, cached))]
pub async fn fetch_event_by_key(
    client: &reqwest::Client,
    base_url: &url::Url,
    key: &GlwEventKey,
    cached: Option<CachedEvent>,
) -> Result<CachedEvent, FetchError> {
    let request = build_request(client, base_url, "key", key.as_str())?;
    fetch_one(client, request, cached).await
}

/// Compose a GET request hitting `<base_url>/glwDataReq.php?<name>=<value>`.
fn build_request(
    client: &reqwest::Client,
    base_url: &url::Url,
    param_name: &str,
    param_value: &str,
) -> Result<reqwest::Request, FetchError> {
    let mut url = base_url.join(GLW_DATA_REQ_PATH)?;
    url.query_pairs_mut().append_pair(param_name, param_value);
    let request = client.get(url).build().map_err(FetchError::from)?;
    Ok(request)
}

/// Common request execution shared by both lookup variants. Honours the
/// `cached` argument as a revalidation hint via `http-cache-semantics`,
/// maps 404 and empty-`{}` responses to `Ok((None, policy))`, and
/// otherwise parses the body as [`GlwEvent`].
async fn fetch_one(
    client: &reqwest::Client,
    request: reqwest::Request,
    cached: Option<CachedEvent>,
) -> Result<CachedEvent, FetchError> {
    if let Some((cached_value, cache_policy)) = cached {
        let now = std::time::SystemTime::now();
        if let http_cache_semantics::BeforeRequest::Fresh(_) =
            cache_policy.before_request(&request, now)
        {
            tracing::debug!("Using cached GLW event/absence");
            return Ok((cached_value, cache_policy));
        }
    }
    let to_send = request
        .try_clone()
        .ok_or(FetchError::FailedToCloneRequest)?;
    let response = client.execute(to_send).await?;
    let cache_policy = http_cache_semantics::CachePolicy::new(&request, &response);
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        tracing::debug!("GLW server returned 404 for event lookup");
        return Ok((None, cache_policy));
    }
    if !status.is_success() {
        let url = response.url().to_string();
        let body = response.text().await.unwrap_or_default();
        return Err(FetchError::BadStatus { status, url, body });
    }
    let body = response.text().await?;
    let trimmed = body.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        tracing::debug!("GLW server returned empty body for event lookup");
        return Ok((None, cache_policy));
    }
    let event: GlwEvent = serde_json::from_str(&body)?;
    Ok((Some(event), cache_policy))
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_base_url_parses() -> Result<(), Box<dyn std::error::Error>> {
        let url = default_base_url()?;
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("globalwind.net"));
        assert_eq!(url.path(), "/glw127/");
        Ok(())
    }

    #[test]
    fn version_override_composes() -> Result<(), Box<dyn std::error::Error>> {
        let url = base_url_for_version("glw128")?;
        assert_eq!(url.path(), "/glw128/");
        Ok(())
    }

    #[test]
    fn url_joins_glw_data_req_path() -> Result<(), Box<dyn std::error::Error>> {
        let base = default_base_url()?;
        let joined = base.join(GLW_DATA_REQ_PATH)?;
        assert_eq!(joined.path(), "/glw127/glwDataReq.php");
        Ok(())
    }
}
