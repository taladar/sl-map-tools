//! HTTP handler for `GET /api/fonts`.
//!
//! Returns the list of TrueType fonts discovered at startup under the
//! configured `fonts_directory`. The render form's font dropdown
//! populates from this endpoint.

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::auth::CurrentUser;
use crate::error::Error;
use crate::fonts::FontInfo;
use crate::state::AppState;

/// Response body for `GET /api/fonts`.
#[derive(Debug, Serialize)]
pub struct ListFontsResponse {
    /// available fonts. `id` is the opaque filename used in render
    /// requests; `name` is the display label.
    pub fonts: Vec<FontInfo>,
}

/// `GET /api/fonts` — list discoverable fonts. Session + CSRF gated
/// alongside the other library routes so unauthenticated clients
/// cannot probe the configured directory.
///
/// # Errors
///
/// Returns the underlying [`Error`] only if the auth extractor
/// rejects the request; the font list itself is in memory and cannot
/// fail.
pub async fn list(
    _user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ListFontsResponse>, Error> {
    let fonts = state.fonts.list();
    Ok(Json(ListFontsResponse { fonts }))
}
