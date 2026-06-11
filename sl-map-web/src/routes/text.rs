//! HTTP handler for `POST /api/text/measure`.
//!
//! Returns the rendered pixel size of a multi-line text block at a given
//! font and pixel size, so the render form can check whether a label will
//! fit a placement slot before the final image is rendered.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::auth::CurrentUser;
use crate::error::Error;
use crate::state::AppState;

/// Request body for `POST /api/text/measure`.
#[derive(Debug, Deserialize)]
pub struct MeasureRequest {
    /// id of the font to measure with (must match one from `GET /api/fonts`).
    pub font_id: String,
    /// font size in pixels.
    pub font_px: f32,
    /// the text, one entry per line.
    pub lines: Vec<String>,
}

/// Response body for `POST /api/text/measure`.
#[derive(Debug, Serialize)]
pub struct MeasureResponse {
    /// rendered width in pixels (the widest line).
    pub width: u32,
    /// rendered height in pixels (all lines stacked).
    pub height: u32,
}

/// `POST /api/text/measure` — measure the rendered size of multi-line text.
/// Uses the same font and metrics the renderer uses, so the result matches
/// what a label would actually occupy.
///
/// # Errors
///
/// Returns an error if the font size is not a positive finite number, the
/// font id is unknown, or the font file cannot be read.
pub async fn measure(
    _user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<MeasureRequest>,
) -> Result<Json<MeasureResponse>, Error> {
    if !(req.font_px.is_finite() && req.font_px > 0f32) {
        return Err(Error::BadRequest(format!(
            "font size must be a positive number of pixels, got {}",
            req.font_px
        )));
    }
    let font_path = state
        .fonts
        .path_for(&req.font_id)
        .ok_or_else(|| Error::BadRequest(format!("unknown font_id `{}`", req.font_id)))?;
    let font = sl_map_apis::text::load_font(font_path)?;
    let scale = ab_glyph::PxScale::from(req.font_px);
    let (width, height) = sl_map_apis::text::measure_text(scale, &font, &req.lines);
    Ok(Json(MeasureResponse { width, height }))
}
