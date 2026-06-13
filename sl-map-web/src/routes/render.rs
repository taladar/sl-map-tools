//! Render endpoints: start a render job, persist it to the library, and
//! return its id. The same UUID is used for the in-memory `JobStore` and the
//! `saved_renders` row, so the existing `/api/render/{id}/*` endpoints
//! (live SSE / in-memory image) and the new `/api/renders/{id}/*` endpoints
//! (persisted) address the same render.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Multipart, State};
use bytes::Bytes;
use chrono::Utc;
use image::GenericImageView as _;
use image::{ImageFormat, Rgba};
use serde::{Deserialize, Serialize};
use sl_map_apis::map_tiles::{Map, MapProgressEvent};
use sl_map_apis::region::usb_notecard_to_grid_rectangle;
use sl_types::map::{
    GridCoordinates, GridRectangle, GridRectangleLike as _, RegionCoordinates, USBNotecard,
    ZoomLevel,
};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::Error;
use crate::jobs::{JobId, JobOutcome, JobState, Metadata, ProgressDto, record_event};
use crate::library::{self, Destination};
use crate::routes::notecards as notecard_routes;
use crate::state::AppState;
use crate::storage;

/// Maximum width or height of a rendered image in pixels. The renderer
/// allocates roughly `4 * max_width * max_height` bytes for the output
/// buffer. This per-side cap is paired with [`MAX_OUTPUT_AREA`] below, which
/// bounds the actual allocation: alone, 32 768 per side would permit a
/// ~4 GiB buffer. Beyond a sanity check it prevents an attacker-supplied
/// `max_width` / `max_height` from driving the server out of memory.
const MAX_OUTPUT_DIMENSION: u32 = 0x8000;

/// Maximum area (`max_width * max_height`) of a rendered image in pixels.
/// 16 384² ≈ 268 M pixels ≈ 1 GiB for the RGBA output buffer — far above any
/// realistic map (the form default is 2048²) yet well below the ~4 GiB a
/// 32 768² request would otherwise allocate. This is the real memory bound and
/// applies on every render *and* read-only preview path (which, unlike the
/// submit path, have no per-user concurrency cap), so a single request — or a
/// burst of preview requests — cannot exhaust memory.
const MAX_OUTPUT_AREA: u64 = 0x4000 * 0x4000;

/// Maximum number of in-progress renders per user. The renderer is
/// serialised on a single map-tile cache, so one user submitting many
/// concurrent jobs would block all other users. Three is a small ceiling
/// that lets a user kick off a couple of variants in parallel without
/// monopolising the worker.
const MAX_CONCURRENT_RENDERS_PER_USER: i64 = 3;

/// Minimum rendered size of a region, in pixels, for the per-region name and
/// grid-coordinate text overlays to be drawn. Below this the text simply does
/// not fit a region and would smear across its neighbours, so both are skipped
/// (the cheap rectangle outline is still drawn). 64 px corresponds to zoom
/// level 3 or lower.
const MIN_PIXELS_PER_REGION_FOR_REGION_LABELS: f32 = 64.0;

/// Maximum number of regions in a render for which the per-region name and
/// grid-coordinate overlays are drawn. Each region name is an individual
/// upstream lookup (cached, but cold on first use), so a huge rectangle would
/// fan out into thousands of requests. With the default 2048 px output and the
/// 64 px-per-region floor above this works out to roughly a 32×32 region area.
const MAX_REGIONS_FOR_REGION_LABELS: usize = 1024;

/// Fraction of a region's rendered pixel size used as the region-label font
/// size, before clamping to [`REGION_LABEL_FONT_MIN_PX`] /
/// [`REGION_LABEL_FONT_MAX_PX`]. Keeps the text proportional to the zoom.
const REGION_LABEL_FONT_FACTOR: f32 = 0.12;

/// Lower clamp for the region-label font size in pixels.
const REGION_LABEL_FONT_MIN_PX: f32 = 8.0;

/// Upper clamp for the region-label font size in pixels.
const REGION_LABEL_FONT_MAX_PX: f32 = 22.0;

/// Padding in pixels between a region's lower-left corner and the text block
/// drawn inside it.
const REGION_LABEL_PADDING: i32 = 3;

/// Colour of the per-region rectangle outline (opaque white; the maps it sits
/// over are land/water so a light hairline reads on both).
const REGION_RECTANGLE_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Which of the optional per-region annotation overlays to draw. All three are
/// independent checkboxes in the UI and may be combined.
#[derive(Debug, Clone, Copy, Default)]
struct RegionOverlayOptions {
    /// draw a hairline rectangle around each region.
    rectangles: bool,
    /// draw each region's name in its lower-left corner.
    names: bool,
    /// draw each region's `(x, y)` grid coordinates (above the name when both
    /// are enabled).
    coordinates: bool,
}

impl RegionOverlayOptions {
    /// whether any overlay at all is requested.
    const fn any(self) -> bool {
        self.rectangles || self.names || self.coordinates
    }

    /// whether any text overlay (name or coordinates) is requested. Text is
    /// the part gated by region size and count.
    const fn any_text(self) -> bool {
        self.names || self.coordinates
    }
}

/// Output format for the rendered image.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// PNG output (default).
    #[default]
    Png,
    /// JPEG output.
    #[serde(alias = "jpg")]
    Jpeg,
}

impl OutputFormat {
    /// Map to the matching `image::ImageFormat`.
    const fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
        }
    }

    /// MIME type for the format.
    const fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// Shared rendering parameters present in both endpoints.
#[derive(Debug, Clone)]
struct CommonParams {
    /// max width of the output image in pixels.
    max_width: u32,
    /// max height of the output image in pixels.
    max_height: u32,
    /// fill colour for missing map tiles (default: leave black).
    missing_map_tile_color: Option<Rgba<u8>>,
    /// fill colour for missing regions (default: water-like).
    missing_region_color: Option<Rgba<u8>>,
    /// output image format.
    format: OutputFormat,
    /// which optional per-region annotation overlays to draw.
    region_overlay: RegionOverlayOptions,
    /// id of the font to draw region names / coordinates with (when enabled).
    region_label_font_id: Option<String>,
}

/// Where a render's GLW overlay should come from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GlwSource {
    /// Reuse a previously-saved `saved_glw_data` row by id.
    SavedId {
        /// the saved row's id.
        glw_data_id: Uuid,
    },
    /// Fetch the event from the GLW server by numeric event id (and
    /// auto-save the resolved JSON to a fresh `saved_glw_data` row).
    EventId {
        /// the numeric event id.
        event_id: u32,
    },
    /// Fetch the event from the GLW server by string event key (and
    /// auto-save).
    EventKey {
        /// the string event key.
        event_key: String,
    },
    /// Parse a JSON document the user pasted into the form (and
    /// auto-save). Advanced / dev path.
    PastedJson {
        /// the raw JSON document, expected to deserialise as a
        /// [`sl_glw::GlwEvent`].
        payload: String,
    },
}

/// Optional per-element style overrides for the GLW overlay. All
/// hex-colour strings are validated server-side via `parse_color`.
/// Absent fields fall back to [`sl_glw::GlwStyle::default`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlwStyleOverrides {
    /// when `true`, draw the dashed margin band around each override.
    #[serde(default)]
    pub margin_band: bool,
    /// optional hex colour for area rectangle outlines.
    #[serde(default)]
    pub area_outline_color: Option<String>,
    /// optional hex colour for circle outlines.
    #[serde(default)]
    pub circle_outline_color: Option<String>,
    /// optional hex colour for the dashed margin band.
    #[serde(default)]
    pub margin_outline_color: Option<String>,
    /// optional hex colour for filled wind arrows.
    #[serde(default)]
    pub wind_color: Option<String>,
    /// optional hex colour for filled current arrows.
    #[serde(default)]
    pub current_color: Option<String>,
    /// optional hex colour for wave glyph strokes.
    #[serde(default)]
    pub wave_color: Option<String>,
    /// optional hex colour for the per-shape label text.
    #[serde(default)]
    pub label_color: Option<String>,
}

/// A free-floating text label to draw in one of the nine placement slots.
/// Independent of the GLW overlay (labels can be added to any render).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLabel {
    /// placement-slot anchor name (`top_left` … `center` … `bottom_right`).
    pub slot: String,
    /// the text, one entry per line. Empty / all-blank labels draw nothing.
    pub lines: Vec<String>,
    /// id of the font to draw this label with (per-snippet). Must match one
    /// returned by `GET /api/fonts`.
    pub font_id: String,
    /// font size in pixels.
    pub font_px: f32,
    /// hex colour string (`#rrggbb`) for the text.
    pub color: String,
    /// horizontal alignment within the slot's free rectangle
    /// (`left` | `center` | `right`); absent → the slot's outward default.
    #[serde(default)]
    pub h_align: Option<String>,
    /// vertical alignment within the slot's free rectangle
    /// (`top` | `center` | `bottom`); absent → the slot's outward default.
    #[serde(default)]
    pub v_align: Option<String>,
    /// the slots this label spans, as combined by the user (each a slot-anchor
    /// name, including [`Self::slot`]). Empty → just [`Self::slot`]. When more
    /// than one, the label grows to the largest free rectangle within exactly
    /// those slots' thirds and reserves every slot listed.
    #[serde(default)]
    pub slots: Vec<String>,
}

/// Default integer scale factor for a [`LogoPlacement`].
const fn default_logo_scale() -> u8 {
    1
}

/// A logo image to composite into one of the nine placement slots, drawn at
/// its native pixel size (optionally integer-doubled) and aligned within the
/// slot's free rectangle like a text label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoPlacement {
    /// placement-slot anchor name (`top_left` … `center` … `bottom_right`).
    pub slot: String,
    /// id of the saved logo (`saved_logos.logo_id`) to draw.
    pub logo_id: Uuid,
    /// integer scale factor applied with nearest-neighbour sampling (no
    /// blur). Must be `1` or `2`.
    #[serde(default = "default_logo_scale")]
    pub scale: u8,
    /// the slots this logo spans, as combined by the user (each a slot-anchor
    /// name, including [`Self::slot`]). Empty → just [`Self::slot`]. When more
    /// than one, the logo's free rectangle is the largest fitting exactly those
    /// slots' thirds and reserves every slot listed.
    #[serde(default)]
    pub slots: Vec<String>,
    /// horizontal alignment within the (single-slot or spanned) free
    /// rectangle; absent → the slot's outward default.
    #[serde(default)]
    pub h_align: Option<String>,
    /// vertical alignment within the free rectangle; absent → the slot's
    /// outward default.
    #[serde(default)]
    pub v_align: Option<String>,
}

/// Per-render GLW configuration accepted by both render endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlwRenderOptions {
    /// where the GLW event comes from.
    pub source: GlwSource,
    /// id of the font to draw labels and the legend with. Must match
    /// one returned by `GET /api/fonts`.
    pub font_id: String,
    /// user-supplied display name for the auto-saved row. When
    /// `None`, the server builds a default name from the source and
    /// fetch timestamp (e.g. `Event 6910 fetched 2026-06-10 15:30 UTC`).
    /// Ignored for the `SavedId` source.
    #[serde(default)]
    pub save_as: Option<String>,
    /// optional style overrides; absent fields use library defaults.
    #[serde(default)]
    pub style: GlwStyleOverrides,
    /// which placement slot the base legend goes in (`top_left` …
    /// `bottom_right`, or `none` to hide it). Absent → `top_left`
    /// (back-compat with renders saved before this field existed).
    #[serde(default)]
    pub legend_slot: Option<String>,
}

/// Body of `POST /api/render/grid-rectangle` (JSON).
#[derive(Debug, Clone, Deserialize)]
pub struct GridRectangleRequest {
    /// lower-left x grid coordinate.
    pub lower_left_x: u16,
    /// lower-left y grid coordinate.
    pub lower_left_y: u16,
    /// upper-right x grid coordinate.
    pub upper_right_x: u16,
    /// upper-right y grid coordinate.
    pub upper_right_y: u16,
    /// max output width in pixels.
    pub max_width: u32,
    /// max output height in pixels.
    pub max_height: u32,
    /// optional hex colour string for missing map tiles.
    #[serde(default)]
    pub missing_map_tile_color: Option<String>,
    /// optional hex colour string for missing regions.
    #[serde(default)]
    pub missing_region_color: Option<String>,
    /// output format.
    #[serde(default)]
    pub format: OutputFormat,
    /// draw a hairline rectangle around each region.
    #[serde(default)]
    pub draw_region_rectangles: bool,
    /// draw each region's name in its lower-left corner.
    #[serde(default)]
    pub draw_region_names: bool,
    /// draw each region's `(x, y)` grid coordinates above its name.
    #[serde(default)]
    pub draw_region_coordinates: bool,
    /// id of the font to draw region names / coordinates with. Required only
    /// when one of those overlays is enabled; must match a `GET /api/fonts` id.
    #[serde(default)]
    pub region_label_font_id: Option<String>,
    /// destination for the saved render. Defaults to the user's personal
    /// library. Format: `"personal"` or `"group:<uuid>"`.
    #[serde(default)]
    pub save_to: Option<String>,
    /// optional GLW (GlobalWind) wind / current / wave overlay. The
    /// auto-saved GLW row inherits this render's `save_to` destination.
    #[serde(default)]
    pub glw: Option<GlwRenderOptions>,
    /// zero or more free-floating text labels to draw in placement slots
    /// (independent of the GLW overlay).
    #[serde(default)]
    pub labels: Vec<TextLabel>,
    /// zero or more logo images to composite in placement slots.
    #[serde(default)]
    pub logos: Vec<LogoPlacement>,
    /// user-defined combined slot groups (each a list of slot-anchor names),
    /// used only by the placement-slots endpoint to report each group's
    /// combined free rectangle. Ignored by the render itself (which takes the
    /// group from each label/logo's `slots`).
    #[serde(default)]
    pub groups: Vec<Vec<String>>,
}

/// Response shape for both render endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct StartedResponse {
    /// the id of the newly created job (also the `saved_renders.render_id`).
    pub job_id: Uuid,
    /// summary of the notecard the render is linked to, if any. For
    /// `usb-notecard` renders this is always populated; the id may differ
    /// from the one the caller submitted if the notecard had to be copied
    /// into the render's scope to satisfy the DB scope-match invariant.
    /// For `grid-rectangle` renders this is omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notecard: Option<NotecardSummary>,
}

/// Minimal notecard descriptor included in [`StartedResponse`] so the UI
/// can update its notecard dropdown in place when the backend copied the
/// source notecard into a new scope.
#[derive(Debug, Clone, Serialize)]
pub struct NotecardSummary {
    /// the id of the notecard the render is linked to. May differ from a
    /// caller-supplied `notecard_id` when an auto-copy happened.
    pub notecard_id: Uuid,
    /// display name, suitable for a dropdown option label.
    pub name: String,
    /// owning scope, in the form `"personal"` or `"group:<uuid>"`.
    pub scope: String,
}

/// Settings JSON stored in `saved_renders.settings_json`. Designed to be
/// fed back into the form for "Regenerate".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SavedRenderSettings {
    /// settings used for a grid-rectangle render.
    GridRectangle(SavedGridRectangleSettings),
    /// settings used for a USB-notecard render.
    UsbNotecard(SavedUsbNotecardSettings),
}

/// Persisted form fields for a grid-rectangle render.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedGridRectangleSettings {
    /// lower-left x grid coordinate.
    pub lower_left_x: u16,
    /// lower-left y grid coordinate.
    pub lower_left_y: u16,
    /// upper-right x grid coordinate.
    pub upper_right_x: u16,
    /// upper-right y grid coordinate.
    pub upper_right_y: u16,
    /// max output width in pixels.
    pub max_width: u32,
    /// max output height in pixels.
    pub max_height: u32,
    /// optional hex colour string for missing map tiles.
    pub missing_map_tile_color: Option<String>,
    /// optional hex colour string for missing regions.
    pub missing_region_color: Option<String>,
    /// output format (`png` / `jpeg`).
    pub format: String,
    /// draw a hairline rectangle around each region.
    #[serde(default)]
    pub draw_region_rectangles: bool,
    /// draw each region's name in its lower-left corner.
    #[serde(default)]
    pub draw_region_names: bool,
    /// draw each region's `(x, y)` grid coordinates above its name.
    #[serde(default)]
    pub draw_region_coordinates: bool,
    /// id of the font used for region names / coordinates.
    #[serde(default)]
    pub region_label_font_id: Option<String>,
    /// GLW overlay used for the render. When set, the carrier is always
    /// `GlwSource::SavedId` so `Regenerate` reliably points at a row in
    /// `saved_glw_data` instead of refetching from the GLW server.
    #[serde(default)]
    pub glw: Option<GlwRenderOptions>,
    /// free-floating text labels drawn on the render.
    #[serde(default)]
    pub labels: Vec<TextLabel>,
    /// logo images composited on the render.
    #[serde(default)]
    pub logos: Vec<LogoPlacement>,
}

/// Persisted form fields for a USB-notecard render.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "this is a flat persisted form-settings record; each bool maps directly to one independent checkbox in the render form"
)]
pub struct SavedUsbNotecardSettings {
    /// the saved notecard the render was launched from.
    pub notecard_id: Uuid,
    /// north-side border padding in whole regions added around the
    /// route's bounding rectangle.
    pub border_north: u16,
    /// south-side border padding in whole regions added around the
    /// route's bounding rectangle.
    pub border_south: u16,
    /// east-side border padding in whole regions added around the
    /// route's bounding rectangle.
    pub border_east: u16,
    /// west-side border padding in whole regions added around the
    /// route's bounding rectangle.
    pub border_west: u16,
    /// canonical `#rrggbb` colour the route polyline was rendered in.
    pub color: String,
    /// max output width in pixels.
    pub max_width: u32,
    /// max output height in pixels.
    pub max_height: u32,
    /// optional hex colour string for missing map tiles.
    pub missing_map_tile_color: Option<String>,
    /// optional hex colour string for missing regions.
    pub missing_region_color: Option<String>,
    /// output format (`png` / `jpeg`).
    pub format: String,
    /// draw a hairline rectangle around each region.
    #[serde(default)]
    pub draw_region_rectangles: bool,
    /// draw each region's name in its lower-left corner.
    #[serde(default)]
    pub draw_region_names: bool,
    /// draw each region's `(x, y)` grid coordinates above its name.
    #[serde(default)]
    pub draw_region_coordinates: bool,
    /// id of the font used for region names / coordinates.
    #[serde(default)]
    pub region_label_font_id: Option<String>,
    /// whether a without-route variant was also produced.
    pub save_without_route: bool,
    /// GLW overlay used for the render. See [`SavedGridRectangleSettings::glw`].
    #[serde(default)]
    pub glw: Option<GlwRenderOptions>,
    /// free-floating text labels drawn on the render.
    #[serde(default)]
    pub labels: Vec<TextLabel>,
    /// logo images composited on the render.
    #[serde(default)]
    pub logos: Vec<LogoPlacement>,
}

/// `POST /api/render/grid-rectangle` — start a render from explicit corners.
///
/// # Errors
///
/// Returns an error if any of the optional hex colour fields fails to parse,
/// the destination is invalid, or the user is not allowed to save to it.
pub async fn grid_rectangle(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<GridRectangleRequest>,
) -> Result<Json<StartedResponse>, Error> {
    validate_dimensions(req.max_width, req.max_height)?;
    let common = CommonParams {
        max_width: req.max_width,
        max_height: req.max_height,
        missing_map_tile_color: req
            .missing_map_tile_color
            .as_deref()
            .map(parse_color)
            .transpose()?,
        missing_region_color: req
            .missing_region_color
            .as_deref()
            .map(parse_color)
            .transpose()?,
        format: req.format,
        region_overlay: RegionOverlayOptions {
            rectangles: req.draw_region_rectangles,
            names: req.draw_region_names,
            coordinates: req.draw_region_coordinates,
        },
        region_label_font_id: req.region_label_font_id.clone(),
    };
    assert_region_overlay_font(
        &state,
        common.region_overlay,
        common.region_label_font_id.as_deref(),
    )?;
    let destination = Destination::parse(req.save_to.as_deref().unwrap_or("personal"))?;
    library::assert_can_write(&state.db, user.user_id, destination).await?;
    assert_under_concurrent_limit(&state.db, user.user_id).await?;
    let logo_ids = validate_logos(&state, user.user_id, destination, &req.logos).await?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    let glw_ctx = req.glw.clone().map(|opts| GlwJobCtx {
        options: opts,
        destination,
        created_by: user.user_id,
    });
    // Reject an over-full placement before persisting anything: the fit check
    // runs on a blank+GLW map, so it needs no map-tile fetch.
    plan_placements(
        &state,
        rect.clone(),
        &common,
        glw_ctx.as_ref(),
        None,
        &req.labels,
        &req.logos,
    )
    .await?;
    let settings = SavedRenderSettings::GridRectangle(SavedGridRectangleSettings {
        lower_left_x: req.lower_left_x,
        lower_left_y: req.lower_left_y,
        upper_right_x: req.upper_right_x,
        upper_right_y: req.upper_right_y,
        max_width: req.max_width,
        max_height: req.max_height,
        missing_map_tile_color: common.missing_map_tile_color.map(hex_from_rgba),
        missing_region_color: common.missing_region_color.map(hex_from_rgba),
        format: format_name(req.format).to_owned(),
        draw_region_rectangles: req.draw_region_rectangles,
        draw_region_names: req.draw_region_names,
        draw_region_coordinates: req.draw_region_coordinates,
        region_label_font_id: req.region_label_font_id.clone(),
        // The worker may rewrite this in place after a successful
        // fresh-fetch so the carrier always lands as `SavedId` at rest
        // (stable Regenerate).
        glw: req.glw.clone(),
        labels: req.labels.clone(),
        logos: req.logos.clone(),
    });
    let render_id = Uuid::new_v4();
    insert_render_row(
        &state,
        render_id,
        destination,
        user.user_id,
        "grid_rectangle",
        None,
        &settings,
        Some(&rect),
    )
    .await?;
    link_render_logos_or_fail(&state, render_id, &logo_ids).await?;
    let (job_id, job) = state.jobs.create_with_id(render_id).await;
    spawn_grid_rectangle_job(
        state, job_id, job, rect, common, glw_ctx, req.labels, req.logos,
    );
    Ok(Json(StartedResponse {
        job_id,
        notecard: None,
    }))
}

/// `POST /api/render/usb-notecard` — start a render from a notecard.
///
/// # Errors
///
/// Returns an error if the multipart form is malformed, the notecard fails
/// to parse, required fields are missing, the destination is invalid, or
/// the user is not allowed to save there.
pub async fn usb_notecard(
    user: CurrentUser,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<StartedResponse>, Error> {
    let parsed = parse_render_form(multipart).await?;
    library::assert_can_write(&state.db, user.user_id, parsed.destination).await?;
    assert_region_overlay_font(
        &state,
        parsed.common.region_overlay,
        parsed.common.region_label_font_id.as_deref(),
    )?;
    assert_under_concurrent_limit(&state.db, user.user_id).await?;
    let logo_ids = validate_logos(&state, user.user_id, parsed.destination, &parsed.logos).await?;

    // Resolve the notecard: reuse an existing one (auto-copied into the
    // render's scope if needed) or persist a freshly uploaded one. The
    // returned summary is what the response surfaces back to the UI so it
    // can update its dropdown if the effective id is new.
    let (notecard, notecard_summary) = resolve_notecard(&state, &user, &parsed).await?;

    let glw_ctx = parsed.glw.clone().map(|opts| GlwJobCtx {
        options: opts,
        destination: parsed.destination,
        created_by: user.user_id,
    });
    // Reject an over-full placement before persisting the render. The route's
    // rectangle is resolved here for the fit check; the job re-resolves it
    // (region lookups are cached) so it can also backfill the notecard bounds.
    let (_bare_rect, rect) = resolve_notecard_rect(&state, &notecard, parsed.borders).await?;
    plan_placements(
        &state,
        rect,
        &parsed.common,
        glw_ctx.as_ref(),
        Some((&notecard, parsed.color)),
        &parsed.labels,
        &parsed.logos,
    )
    .await?;

    let settings = SavedRenderSettings::UsbNotecard(SavedUsbNotecardSettings {
        notecard_id: notecard_summary.notecard_id,
        border_north: parsed.borders.0,
        border_south: parsed.borders.1,
        border_east: parsed.borders.2,
        border_west: parsed.borders.3,
        color: hex_from_rgba(parsed.color),
        max_width: parsed.common.max_width,
        max_height: parsed.common.max_height,
        missing_map_tile_color: parsed.common.missing_map_tile_color.map(hex_from_rgba),
        missing_region_color: parsed.common.missing_region_color.map(hex_from_rgba),
        format: format_name(parsed.common.format).to_owned(),
        draw_region_rectangles: parsed.common.region_overlay.rectangles,
        draw_region_names: parsed.common.region_overlay.names,
        draw_region_coordinates: parsed.common.region_overlay.coordinates,
        region_label_font_id: parsed.common.region_label_font_id.clone(),
        save_without_route: parsed.with_without_route,
        // The worker may rewrite this in place after a successful
        // fresh-fetch so the carrier always lands as `SavedId` at rest.
        glw: parsed.glw.clone(),
        labels: parsed.labels.clone(),
        logos: parsed.logos.clone(),
    });

    let render_id = Uuid::new_v4();
    insert_render_row(
        &state,
        render_id,
        parsed.destination,
        user.user_id,
        "usb_notecard",
        Some(notecard_summary.notecard_id),
        &settings,
        None,
    )
    .await?;
    link_render_logos_or_fail(&state, render_id, &logo_ids).await?;

    let (job_id, job) = state.jobs.create_with_id(render_id).await;
    spawn_usb_notecard_job(
        state,
        job_id,
        job,
        notecard_summary.notecard_id,
        notecard,
        parsed.borders,
        parsed.color,
        parsed.common,
        parsed.with_without_route,
        glw_ctx,
        parsed.labels,
        parsed.logos,
    );
    Ok(Json(StartedResponse {
        job_id,
        notecard: Some(notecard_summary),
    }))
}

// =====================================================================
// Free-placement-slot detection — read-only preview, no DB writes.
//
// Reports which of nine fixed anchor positions (the four corners, the four
// side midpoints and the centre) are free of overlay content (route + GLW
// shapes/labels) so the UI can offer them as placement targets for a legend,
// logo or label *before* the final render. Computed by drawing the overlays
// onto a blank (no base tiles) map and measuring the empty space; see
// [`sl_map_apis::coverage`].
// =====================================================================

/// An axis-aligned rectangle in image pixel coordinates (origin top-left).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PixelRectDto {
    /// x coordinate of the left edge in pixels.
    pub x: u32,
    /// y coordinate of the top edge in pixels.
    pub y: u32,
    /// width in pixels.
    pub width: u32,
    /// height in pixels.
    pub height: u32,
}

/// One of the nine candidate placement slots and how much free space it has.
#[derive(Debug, Clone, Serialize)]
pub struct SlotDto {
    /// the slot name (`top_left`, `top_center`, …, `center`, …,
    /// `bottom_right`).
    pub slot: &'static str,
    /// whether the slot itself is free of overlay content.
    pub available: bool,
    /// the largest empty rectangle that can be placed anchored here, confined to
    /// this slot's own third of the map so the nine slots never overlap, or
    /// `null` when the slot's third has no free space at the anchor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_rect: Option<PixelRectDto>,
    /// width of [`Self::free_rect`] in pixels (`0` when covered).
    pub free_width: u32,
    /// height of [`Self::free_rect`] in pixels (`0` when covered).
    pub free_height: u32,
    /// fraction (`0.0..=1.0`) of the local third-of-the-map region around the
    /// anchor that is covered, as a density hint.
    pub occupied_fraction: f32,
    /// orthogonally adjacent anchors that share one contiguous free area with
    /// this anchor, so they can be combined for a larger element.
    pub connected_neighbours: Vec<&'static str>,
}

/// The combined free rectangle for one user-defined slot group (two or more
/// slots joined together), so the client can draw and fit-check it.
#[derive(Debug, Clone, Serialize)]
pub struct GroupDto {
    /// the slot-anchor names making up this group (the first is the primary
    /// anchor used for alignment).
    pub slots: Vec<&'static str>,
    /// whether the group has any free rectangle.
    pub available: bool,
    /// the largest free rectangle within exactly these slots' thirds, or `null`
    /// when the group is fully covered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_rect: Option<PixelRectDto>,
    /// width of [`Self::free_rect`] in pixels (`0` when covered).
    pub free_width: u32,
    /// height of [`Self::free_rect`] in pixels (`0` when covered).
    pub free_height: u32,
}

/// Response shape for the `placement-slots` endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct PlacementSlotsResponse {
    /// width in pixels of the image the final render will produce.
    pub image_width: u32,
    /// height in pixels of the image the final render will produce.
    pub image_height: u32,
    /// the nine candidate anchors, in reading order.
    pub slots: Vec<SlotDto>,
    /// combined rectangles for the requested multi-slot groups, in request
    /// order (empty when none were requested).
    pub groups: Vec<GroupDto>,
}

/// Reduce a drawn-on blank map to the nine-slot placement report, plus a
/// combined rectangle for each requested multi-slot `group`.
fn compute_placement_slots(
    map: &Map,
    groups: &[Vec<sl_map_apis::coverage::PlacementSlot>],
) -> PlacementSlotsResponse {
    use sl_map_apis::coverage::PlacementSlot;
    let image = sl_map_apis::map_tiles::MapLike::image(map);
    let (image_width, image_height) = image.dimensions();
    let grid = sl_map_apis::coverage::OccupancyGrid::from_map(
        map,
        sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
    );
    let slots = grid
        .evaluate_slots()
        .into_iter()
        .map(|info| SlotDto {
            slot: info.slot.as_str(),
            available: info.available,
            free_rect: info.free_rect.map(|r| PixelRectDto {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            }),
            free_width: info.free_size.0,
            free_height: info.free_size.1,
            occupied_fraction: info.occupied_fraction,
            connected_neighbours: info
                .connected_neighbours
                .into_iter()
                .map(PlacementSlot::as_str)
                .collect(),
        })
        .collect();
    let group_dtos = groups
        .iter()
        .filter(|g| g.len() > 1)
        .map(|g| {
            let rect = grid.subset_rect(g);
            GroupDto {
                slots: g.iter().map(|s| s.as_str()).collect(),
                available: rect.is_some(),
                free_rect: rect.map(|r| PixelRectDto {
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                }),
                free_width: rect.map_or(0, |r| r.width),
                free_height: rect.map_or(0, |r| r.height),
            }
        })
        .collect();
    PlacementSlotsResponse {
        image_width,
        image_height,
        slots,
        groups: group_dtos,
    }
}

/// Parse client-supplied slot groups (lists of slot-anchor names) into
/// [`PlacementSlot`] vectors, rejecting unknown names and empty groups.
fn parse_groups(
    raw: &[Vec<String>],
) -> Result<Vec<Vec<sl_map_apis::coverage::PlacementSlot>>, Error> {
    raw.iter()
        .map(|g| {
            if g.is_empty() {
                return Err(Error::BadRequest(
                    "a placement group must not be empty".to_owned(),
                ));
            }
            g.iter()
                .map(|name| {
                    name.parse::<sl_map_apis::coverage::PlacementSlot>()
                        .map_err(|err: sl_map_apis::coverage::ParsePlacementSlotError| {
                            Error::BadRequest(err.to_string())
                        })
                })
                .collect()
        })
        .collect()
}

/// Resolve a GLW source to an event WITHOUT any persistence (unlike
/// [`resolve_glw_event`], which inserts a `saved_glw_data` row for fresh-fetch
/// sources). Used by the read-only placement-slots preview. `SavedId` still
/// goes through the same read-permission gate.
async fn resolve_glw_event_readonly(
    state: &AppState,
    user_id: Uuid,
    source: &GlwSource,
) -> Result<Option<sl_glw::GlwEvent>, Error> {
    match source {
        GlwSource::SavedId { glw_data_id } => {
            let row =
                crate::library::assert_can_read_glw_data(&state.db, user_id, *glw_data_id).await?;
            Ok(Some(serde_json::from_str(&row.payload_json)?))
        }
        GlwSource::EventId { event_id } => {
            let id = sl_glw::EventId::new(*event_id);
            let mut cache = state.glw_event_cache.lock().await;
            Ok(cache.get_event_by_id(id).await?)
        }
        GlwSource::EventKey { event_key } => {
            let key = sl_glw::GlwEventKey::new(event_key);
            let mut cache = state.glw_event_cache.lock().await;
            Ok(cache.get_event_by_key(&key).await?)
        }
        GlwSource::PastedJson { payload } => Ok(Some(serde_json::from_str(payload)?)),
    }
}

/// Draw the GLW overlay onto `map` for occupancy purposes only: the legend is
/// deliberately excluded (it is a *candidate* element to be placed, not a
/// constraint), so it does not block its own slot. Per-shape labels are kept
/// because they mark real overlay positions.
async fn apply_glw_overlay_readonly(
    state: &AppState,
    user_id: Uuid,
    options: &GlwRenderOptions,
    map: &mut Map,
) -> Result<(), Error> {
    use sl_glw::MapLikeGlwExt as _;
    let font_path = state
        .fonts
        .path_for(&options.font_id)
        .ok_or_else(|| Error::BadRequest(format!("unknown font_id `{}`", options.font_id)))?;
    let font = sl_map_apis::text::load_font(font_path)?;
    let Some(event) = resolve_glw_event_readonly(state, user_id, &options.source).await? else {
        tracing::warn!("GLW event not found; placement slots computed without overlay");
        return Ok(());
    };
    // The legend is excluded from occupancy (it is a candidate element, not a
    // constraint), so its slot does not matter here.
    let mut style = build_glw_style(&options.style, None)?;
    style.legend_position = None;
    map.draw_glw_event_with_font(&event, &style, &font)?;
    Ok(())
}

/// `POST /api/render/placement-slots/grid-rectangle` — report the free
/// placement slots for an explicit-corner render (optional GLW overlay), with
/// no route. Read-only: nothing is persisted.
///
/// # Errors
///
/// Returns an error if the dimensions are invalid, the GLW colours fail to
/// parse, or a referenced GLW row cannot be read.
pub async fn free_placement_slots_grid_rectangle(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<GridRectangleRequest>,
) -> Result<Json<PlacementSlotsResponse>, Error> {
    validate_dimensions(req.max_width, req.max_height)?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    let groups = parse_groups(&req.groups)?;
    let mut map = Map::blank_fit(rect, req.max_width, req.max_height)?;
    if let Some(glw) = req.glw.as_ref() {
        apply_glw_overlay_readonly(&state, user.user_id, glw, &mut map).await?;
    }
    Ok(Json(compute_placement_slots(&map, &groups)))
}

/// `POST /api/render/placement-slots/usb-notecard` — report the free
/// placement slots for a notecard render (route + optional GLW overlay).
/// Read-only: the notecard is parsed/loaded but never copied or persisted,
/// and no GLW row is inserted.
///
/// # Errors
///
/// Returns an error if the form is malformed, the notecard cannot be parsed
/// or read, a region cannot be resolved, or the GLW overlay fails to resolve.
pub async fn free_placement_slots_usb_notecard(
    user: CurrentUser,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<PlacementSlotsResponse>, Error> {
    let parsed = parse_render_form(multipart).await?;
    // Read-only notecard resolution (no auto-copy, no persistence).
    let notecard: USBNotecard = match &parsed.notecard_source {
        NotecardSource::Fresh { text, .. } => text.parse()?,
        NotecardSource::Existing { notecard_id } => {
            let row =
                crate::library::assert_can_read_notecard(&state.db, user.user_id, *notecard_id)
                    .await?;
            row.body.parse()?
        }
    };
    let (border_north, border_south, border_east, border_west) = parsed.borders;
    let rect = {
        let mut region = state.region_cache.lock().await;
        usb_notecard_to_grid_rectangle(&mut region, &notecard).await?
    }
    .expanded_west(border_west)
    .expanded_east(border_east)
    .expanded_south(border_south)
    .expanded_north(border_north);
    let mut map = Map::blank_fit(rect, parsed.common.max_width, parsed.common.max_height)?;
    // Same layering as the real render: GLW under the route.
    if let Some(glw) = parsed.glw.as_ref() {
        apply_glw_overlay_readonly(&state, user.user_id, glw, &mut map).await?;
    }
    {
        let mut region = state.region_cache.lock().await;
        map.draw_route_with_progress(&mut region, &notecard, parsed.color, None)
            .await?;
    }
    let groups = parse_groups(&parsed.groups)?;
    Ok(Json(compute_placement_slots(&map, &groups)))
}

/// Body of `POST /api/render/glw-preview` (JSON).
#[derive(Debug, Clone, Deserialize)]
pub struct GlwPreviewRequest {
    /// lower-left x grid coordinate of the final-image rectangle.
    pub lower_left_x: u16,
    /// lower-left y grid coordinate of the final-image rectangle.
    pub lower_left_y: u16,
    /// upper-right x grid coordinate of the final-image rectangle.
    pub upper_right_x: u16,
    /// upper-right y grid coordinate of the final-image rectangle.
    pub upper_right_y: u16,
    /// zoom level (1–8) the client is compositing the preview tiles at. The
    /// overlay is rasterised at this same zoom so its shapes line up 1:1 with
    /// the displayed tiles once the PNG is dropped into the final-image
    /// bounds, and so the returned image stays small.
    pub zoom: u8,
    /// the GLW overlay to draw.
    pub glw: GlwRenderOptions,
}

/// `POST /api/render/glw-preview` — rasterise just the GLW overlay onto a
/// transparent image matching the final-image bounds, so the client-side
/// preview can composite it over the map tiles. Read-only: nothing is
/// persisted (no `saved_glw_data` row is inserted).
///
/// The overlay draws the geographic GLW content — shapes and their per-shape
/// labels — at the preview's zoom level rather than the final render's, so the
/// returned PNG is small and aligns with the preview tiles. The base legend is
/// deliberately omitted: like [`apply_glw_overlay_readonly`] it is a candidate
/// placement element handled separately by the placement-slot logic, not part
/// of the geographic overlay. When the GLW event cannot be resolved the
/// response is a fully transparent PNG, leaving the tiles unchanged.
///
/// # Errors
///
/// Returns an error if the zoom level is out of range, the font is unknown,
/// the GLW colours fail to parse, or a referenced GLW row cannot be read.
pub async fn glw_preview(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<GlwPreviewRequest>,
) -> Result<axum::response::Response, Error> {
    use axum::response::IntoResponse as _;
    use sl_glw::MapLikeGlwExt as _;
    let zoom = ZoomLevel::try_new(req.zoom)
        .map_err(|err| Error::BadRequest(format!("invalid zoom: {err}")))?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    let mut map = Map::blank(rect, zoom);
    // Resolve the font first so a missing-font request fails fast.
    let font_path = state
        .fonts
        .path_for(&req.glw.font_id)
        .ok_or_else(|| Error::BadRequest(format!("unknown font_id `{}`", req.glw.font_id)))?;
    let font = sl_map_apis::text::load_font(font_path)?;
    // Read-only resolution: unlike the real render this never persists a row.
    if let Some(event) = resolve_glw_event_readonly(&state, user.user_id, &req.glw.source).await? {
        // The legend is excluded from the overlay: it is a *candidate*
        // placement element handled separately by the placement-slot logic
        // (see `apply_glw_overlay_readonly`), not part of the geographic
        // overlay the preview composites over the tiles. Only the shapes and
        // their per-shape labels are drawn.
        let mut style = build_glw_style(&req.glw.style, None)?;
        style.legend_position = None;
        map.draw_glw_event_with_font(&event, &style, &font)?;
    } else {
        tracing::warn!("GLW event not found; preview overlay is fully transparent");
    }
    // PNG keeps the transparent background so only the overlay composites over
    // the client's tiles.
    let image = encode_map(&map, OutputFormat::Png)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], image).into_response())
}

/// `POST /api/render/glw-legend-preview` — rasterise just the GLW base legend
/// onto a transparent image the size of the final render, so the client-side
/// preview can drop it into the final-image bounds rectangle and show the
/// legend exactly where — and at the size — the final render will place it.
///
/// Unlike [`glw_preview`] this draws ONLY the legend (no geographic shapes or
/// per-shape labels), and it renders at the final-image resolution
/// (`Map::blank_fit` with the request's `max_width`/`max_height`) rather than
/// the preview zoom, so the legend's anchored position and its fixed font size
/// match the real render once the client scales the PNG into the bounds
/// rectangle. The request reuses [`GridRectangleRequest`]; only the corners,
/// `max_width`/`max_height` and `glw` (whose `legend_slot` chooses the slot)
/// are consulted. When no GLW overlay is requested, the legend slot is `none`,
/// or the GLW event cannot be resolved, the response is a fully transparent
/// PNG, leaving the tiles unchanged.
///
/// # Errors
///
/// Returns an error if the dimensions are invalid, the font is unknown, the
/// GLW colours fail to parse, or a referenced GLW row cannot be read.
pub async fn glw_legend_preview(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<GridRectangleRequest>,
) -> Result<axum::response::Response, Error> {
    use axum::response::IntoResponse as _;
    use sl_glw::MapLikeGlwExt as _;
    validate_dimensions(req.max_width, req.max_height)?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    // Final-image resolution so the legend matches the real render exactly.
    let mut map = Map::blank_fit(rect, req.max_width, req.max_height)?;
    if let Some(glw) = req.glw.as_ref() {
        // Resolve the font first so a missing-font request fails fast.
        let font_path = state
            .fonts
            .path_for(&glw.font_id)
            .ok_or_else(|| Error::BadRequest(format!("unknown font_id `{}`", glw.font_id)))?;
        let font = sl_map_apis::text::load_font(font_path)?;
        let style = build_glw_style(&glw.style, glw.legend_slot.as_deref())?;
        // Draw only the legend, at its chosen slot. `legend_position` is `None`
        // when the slot is `none`, so skip the lookup/draw entirely then.
        if style.legend_position.is_some() {
            if let Some(event) =
                resolve_glw_event_readonly(&state, user.user_id, &glw.source).await?
            {
                map.draw_glw_base_legend(&event.base, &style, &font)?;
            } else {
                tracing::warn!("GLW event not found; legend preview is fully transparent");
            }
        }
    }
    // PNG keeps the transparent background so only the legend composites over
    // the client's tiles.
    let image = encode_map(&map, OutputFormat::Png)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], image).into_response())
}

/// One already-resolved route waypoint in a [`RoutePreviewRequest`]. The
/// region has already been resolved to grid coordinates by the client's
/// `/api/notecard/derive-rectangle` call, so the preview endpoint needs no
/// region lookup (no DB / network). Mirrors the fields of
/// [`crate::routes::notecards`]'s `ResolvedWaypoint`; `z` is irrelevant to the
/// pixel mapping and is omitted.
#[derive(Debug, Clone, Deserialize)]
pub struct RoutePreviewWaypoint {
    /// resolved x grid coordinate of the region.
    pub region_x: u16,
    /// resolved y grid coordinate of the region.
    pub region_y: u16,
    /// in-region x coordinate of the waypoint (metres).
    pub x: f32,
    /// in-region y coordinate of the waypoint (metres).
    pub y: f32,
}

/// Body of `POST /api/render/route-preview` (JSON).
#[derive(Debug, Clone, Deserialize)]
pub struct RoutePreviewRequest {
    /// lower-left x grid coordinate of the final-image rectangle.
    pub lower_left_x: u16,
    /// lower-left y grid coordinate of the final-image rectangle.
    pub lower_left_y: u16,
    /// upper-right x grid coordinate of the final-image rectangle.
    pub upper_right_x: u16,
    /// upper-right y grid coordinate of the final-image rectangle.
    pub upper_right_y: u16,
    /// final-image fit width in pixels (so the route is rasterised at the same
    /// resolution the real render uses).
    pub max_width: u32,
    /// final-image fit height in pixels.
    pub max_height: u32,
    /// route colour as `#rrggbb`.
    pub color: String,
    /// the already-resolved waypoints, in order.
    pub waypoints: Vec<RoutePreviewWaypoint>,
}

/// `POST /api/render/route-preview` — rasterise just the route onto a
/// transparent image at the final-image resolution, so the client-side preview
/// can composite it over the map tiles exactly as the real render draws it.
/// Read-only: nothing is persisted.
///
/// The route is drawn with the *same* code the final image uses
/// ([`Map::draw_pixel_waypoint_route`] — Catmull-Rom spline + per-waypoint
/// arrows + the route colour), onto a [`Map::blank_fit`] map sized for the
/// request's `max_width` / `max_height`. The client drops the returned PNG into
/// the preview's bounds rectangle (scaling it down), so the preview route is a
/// pixel-faithful, merely-downscaled copy of what the output will contain —
/// including correct arrow/line proportions and the same edge clipping. Unlike
/// [`Map::draw_route_with_progress`] the waypoints arrive already resolved to
/// grid coordinates, so this performs no region lookup.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] if the dimensions are invalid, the colour
/// fails to parse, or a waypoint falls outside the rectangle; propagates any
/// error from the spline rasterisation or PNG encoding.
pub async fn route_preview(
    _user: CurrentUser,
    Json(req): Json<RoutePreviewRequest>,
) -> Result<axum::response::Response, Error> {
    use axum::response::IntoResponse as _;
    use sl_map_apis::map_tiles::MapLike as _;
    validate_dimensions(req.max_width, req.max_height)?;
    let color = parse_color(&req.color)?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    // Final-image resolution so the route matches the real render exactly once
    // the client scales the PNG into the bounds rectangle.
    let mut map = Map::blank_fit(rect, req.max_width, req.max_height)?;
    let mut pixel_waypoints = Vec::with_capacity(req.waypoints.len());
    for waypoint in &req.waypoints {
        let grid = GridCoordinates::new(waypoint.region_x, waypoint.region_y);
        let region_coordinates = RegionCoordinates::new(waypoint.x, waypoint.y, 0f32);
        let (x, y) = map
            .pixel_coordinates_for_coordinates(&grid, &region_coordinates)
            .ok_or_else(|| {
                Error::BadRequest("a route waypoint falls outside the rectangle".to_owned())
            })?;
        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            reason = "matches draw_route_with_progress: pixel coordinates never approach 2^23"
        )]
        pixel_waypoints.push((x as f32, y as f32));
    }
    map.draw_pixel_waypoint_route(&pixel_waypoints, color)
        .map_err(|err| Error::BadRequest(format!("route rasterisation failed: {err}")))?;
    // PNG keeps the transparent background so only the route composites over the
    // client's tiles.
    let image = encode_map(&map, OutputFormat::Png)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], image).into_response())
}

/// Body of `POST /api/render/region-overlay-preview` (JSON).
#[derive(Debug, Clone, Deserialize)]
pub struct RegionOverlayPreviewRequest {
    /// lower-left x grid coordinate of the final-image rectangle.
    pub lower_left_x: u16,
    /// lower-left y grid coordinate of the final-image rectangle.
    pub lower_left_y: u16,
    /// upper-right x grid coordinate of the final-image rectangle.
    pub upper_right_x: u16,
    /// upper-right y grid coordinate of the final-image rectangle.
    pub upper_right_y: u16,
    /// final-image fit width in pixels (so the overlay is rasterised at the
    /// same resolution — and thus the same per-region pixel size — the real
    /// render uses, keeping the size gate consistent between preview and final).
    pub max_width: u32,
    /// final-image fit height in pixels.
    pub max_height: u32,
    /// draw a hairline rectangle around each region.
    #[serde(default)]
    pub draw_region_rectangles: bool,
    /// draw each region's name in its lower-left corner.
    #[serde(default)]
    pub draw_region_names: bool,
    /// draw each region's `(x, y)` grid coordinates above its name.
    #[serde(default)]
    pub draw_region_coordinates: bool,
    /// id of the font to draw region names / coordinates with.
    #[serde(default)]
    pub region_label_font_id: Option<String>,
    /// optional hex colour for the missing-region fill. When set (the user has
    /// "Fill missing regions" enabled) and region names are being looked up,
    /// regions the lookup reports as non-existent are painted with this colour,
    /// reusing the lookup result the names need anyway. Absent → no fill.
    #[serde(default)]
    pub missing_region_color: Option<String>,
}

/// `POST /api/render/region-overlay-preview` — rasterise the optional
/// per-region annotation overlay (rectangles, names, grid coordinates) onto a
/// transparent image at the final-image resolution, so the client-side preview
/// can composite it into the bounds rectangle exactly where — and gated exactly
/// as — the final render draws it. Read-only: nothing is persisted (region
/// names are resolved through the shared cache only).
///
/// Resolving region names is the slow part (one cached-but-cold lookup per
/// region), so the response is a streaming `application/x-ndjson` body rather
/// than a bare PNG: one JSON object per line, the region-name progress events
/// (`region_names_planned` / `region_name_resolved`, same shape as the render
/// SSE) as they happen, then a terminal line — `{"type":"image","data":"<base64
/// png>"}` on success or `{"type":"error","message":…}` on failure. This lets
/// the preview show a live `region names: N / M` counter for the work it (not
/// the later render, which finds the names already cached) actually performs.
///
/// # Errors
///
/// Returns an error (before streaming starts) if the dimensions are invalid or a
/// text overlay is requested without a (known) font. Failures during rendering
/// are reported as the terminal `error` line within the stream.
pub async fn region_overlay_preview(
    _user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<RegionOverlayPreviewRequest>,
) -> Result<axum::response::Response, Error> {
    use axum::response::IntoResponse as _;
    use futures::StreamExt as _;
    validate_dimensions(req.max_width, req.max_height)?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    let opts = RegionOverlayOptions {
        rectangles: req.draw_region_rectangles,
        names: req.draw_region_names,
        coordinates: req.draw_region_coordinates,
    };
    // Fail fast with a normal HTTP error (before any streaming) when a text
    // overlay is requested without a usable font.
    assert_region_overlay_font(&state, opts, req.region_label_font_id.as_deref())?;
    let font_id = req.region_label_font_id.clone();
    let missing_region_color = req
        .missing_region_color
        .as_deref()
        .map(parse_color)
        .transpose()?;
    let (max_width, max_height) = (req.max_width, req.max_height);

    // The worker streams NDJSON lines into this channel; the response body drains
    // it. A small buffer is fine — `forward_region_overlay_progress` applies
    // backpressure with `.send().await`.
    let (line_tx, line_rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    drop(tokio::spawn(run_region_overlay_preview_stream(
        state,
        rect,
        opts,
        font_id,
        missing_region_color,
        max_width,
        max_height,
        line_tx,
    )));
    let stream = ReceiverStream::new(line_rx).map(Ok::<Bytes, std::convert::Infallible>);
    let body = axum::body::Body::from_stream(stream);
    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        body,
    )
        .into_response())
}

/// Worker for [`region_overlay_preview`]: build the overlay (streaming
/// region-name progress as NDJSON lines into `line_tx`) and emit a terminal
/// `image` (base64 PNG) or `error` line. Best-effort throughout — if the client
/// disconnects, the line sends fail and the worker simply stops.
#[expect(
    clippy::too_many_arguments,
    reason = "a one-shot helper carrying the preview request fields into the streaming task"
)]
async fn run_region_overlay_preview_stream(
    state: AppState,
    rect: GridRectangle,
    opts: RegionOverlayOptions,
    font_id: Option<String>,
    missing_region_color: Option<Rgba<u8>>,
    max_width: u32,
    max_height: u32,
    line_tx: tokio::sync::mpsc::Sender<Bytes>,
) {
    let (ev_tx, ev_rx) = tokio::sync::mpsc::channel::<MapProgressEvent>(256);
    let forwarder = tokio::spawn(forward_region_overlay_progress(ev_rx, line_tx.clone()));
    let result = async {
        let mut map = Map::blank_fit(rect, max_width, max_height)?;
        apply_region_overlay(
            &state,
            opts,
            font_id.as_deref(),
            missing_region_color,
            &mut map,
            Some(&ev_tx),
        )
        .await?;
        encode_map(&map, OutputFormat::Png)
    }
    .await;
    drop(ev_tx);
    let _join = forwarder.await;
    let final_line = match result {
        Ok(png) => {
            use base64::Engine as _;
            let data = base64::engine::general_purpose::STANDARD.encode(&png);
            serde_json::json!({ "type": "image", "data": data }).to_string()
        }
        Err(err) => serde_json::to_string(&ProgressDto::Error {
            message: err.to_string(),
        })
        .unwrap_or_else(|_| {
            r#"{"type":"error","message":"region overlay preview failed"}"#.to_owned()
        }),
    };
    drop(line_tx.send(Bytes::from(format!("{final_line}\n"))).await);
}

/// Convert region-name `MapProgressEvent`s into NDJSON lines (one per line) on
/// the response channel, reusing [`ProgressDto`] so the wire shape matches the
/// render SSE. Stops when the event channel closes or the client disconnects.
async fn forward_region_overlay_progress(
    mut ev_rx: tokio::sync::mpsc::Receiver<MapProgressEvent>,
    line_tx: tokio::sync::mpsc::Sender<Bytes>,
) {
    while let Some(event) = ev_rx.recv().await {
        let Ok(json) = serde_json::to_string(&ProgressDto::from(event)) else {
            continue;
        };
        if line_tx
            .send(Bytes::from(format!("{json}\n")))
            .await
            .is_err()
        {
            break;
        }
    }
}

/// Plan the labels + logos against `occupancy` (an overlay-only map carrying
/// the route + GLW shapes) and draw them onto a fresh transparent map of the
/// same final-image dimensions, returning the PNG. Used by the placement
/// preview endpoints: it runs the very same planning the real render does, so
/// the preview shows each label/logo exactly where the final image places it.
#[expect(
    clippy::too_many_arguments,
    reason = "a one-shot helper shared by the two placement-preview endpoints"
)]
async fn render_placement_elements_png(
    state: &AppState,
    occupancy: &Map,
    target_rect: GridRectangle,
    max_width: u32,
    max_height: u32,
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    labels: &[TextLabel],
    logos: &[LogoPlacement],
) -> Result<Bytes, Error> {
    let (label_draws, label_slots) =
        plan_labels(&state.fonts, labels, legend_slot, &[], occupancy)?;
    let (logo_draws, _) = plan_logos(state, logos, legend_slot, &label_slots, occupancy).await?;
    let mut target = Map::blank_fit(target_rect, max_width, max_height)?;
    execute_labels(&label_draws, &mut target);
    execute_logos(&logo_draws, &mut target);
    encode_map(&target, OutputFormat::Png)
}

/// `POST /api/render/placement-preview/grid-rectangle` — rasterise the text
/// labels and logos of an explicit-corner render onto a transparent PNG at the
/// final-image resolution, so the client preview can composite them into the
/// bounds rectangle exactly where the final render will place them. Free space
/// is measured on an overlay-only map (GLW shapes; no route on this path), as
/// the real render does. Read-only: nothing is persisted.
///
/// # Errors
///
/// Returns an error if the dimensions are invalid, a placement overflows or
/// clashes, a font/logo is unknown, or the GLW overlay fails to resolve.
pub async fn placement_preview_grid_rectangle(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<GridRectangleRequest>,
) -> Result<axum::response::Response, Error> {
    use axum::response::IntoResponse as _;
    validate_dimensions(req.max_width, req.max_height)?;
    // Gate read access before compositing — `plan_logos` loads the file by id
    // with no authorization (the submit path relies on `validate_logos`, which
    // the preview path does not run).
    assert_can_read_logos(&state, user.user_id, &req.logos).await?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    let mut occ = Map::blank_fit(rect.clone(), req.max_width, req.max_height)?;
    let legend_slot = if let Some(glw) = req.glw.as_ref() {
        apply_glw_overlay_readonly(&state, user.user_id, glw, &mut occ).await?;
        legend_position_from_slot(glw.legend_slot.as_deref())?
    } else {
        None
    };
    let png = render_placement_elements_png(
        &state,
        &occ,
        rect,
        req.max_width,
        req.max_height,
        legend_slot,
        &req.labels,
        &req.logos,
    )
    .await?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], png).into_response())
}

/// `POST /api/render/placement-preview/usb-notecard` — same as
/// [`placement_preview_grid_rectangle`] but for a notecard render: free space
/// is measured on an overlay-only map carrying the route plus the GLW shapes.
/// Read-only: the notecard is parsed/loaded but never copied or persisted.
///
/// # Errors
///
/// Returns an error if the form is malformed, the notecard cannot be parsed or
/// read, a region cannot be resolved, a placement overflows or clashes, or the
/// GLW overlay fails to resolve.
pub async fn placement_preview_usb_notecard(
    user: CurrentUser,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<axum::response::Response, Error> {
    use axum::response::IntoResponse as _;
    let parsed = parse_render_form(multipart).await?;
    // Gate read access before compositing — `plan_logos` loads the file by id
    // with no authorization (the submit path relies on `validate_logos`, which
    // the preview path does not run).
    assert_can_read_logos(&state, user.user_id, &parsed.logos).await?;
    let notecard: USBNotecard = match &parsed.notecard_source {
        NotecardSource::Fresh { text, .. } => text.parse()?,
        NotecardSource::Existing { notecard_id } => {
            let row =
                crate::library::assert_can_read_notecard(&state.db, user.user_id, *notecard_id)
                    .await?;
            row.body.parse()?
        }
    };
    let (border_north, border_south, border_east, border_west) = parsed.borders;
    let rect = {
        let mut region = state.region_cache.lock().await;
        usb_notecard_to_grid_rectangle(&mut region, &notecard).await?
    }
    .expanded_west(border_west)
    .expanded_east(border_east)
    .expanded_south(border_south)
    .expanded_north(border_north);
    // Same overlay-only occupancy as the real notecard render: GLW shapes
    // (legend excluded) then the route.
    let mut occ = Map::blank_fit(
        rect.clone(),
        parsed.common.max_width,
        parsed.common.max_height,
    )?;
    let legend_slot = if let Some(glw) = parsed.glw.as_ref() {
        apply_glw_overlay_readonly(&state, user.user_id, glw, &mut occ).await?;
        legend_position_from_slot(glw.legend_slot.as_deref())?
    } else {
        None
    };
    {
        let mut region = state.region_cache.lock().await;
        occ.draw_route_with_progress(&mut region, &notecard, parsed.color, None)
            .await?;
    }
    let png = render_placement_elements_png(
        &state,
        &occ,
        rect,
        parsed.common.max_width,
        parsed.common.max_height,
        legend_slot,
        &parsed.labels,
        &parsed.logos,
    )
    .await?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], png).into_response())
}

/// Map a [`OutputFormat`] to its lowercase name used in the persisted
/// settings JSON.
const fn format_name(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Png => "png",
        OutputFormat::Jpeg => "jpeg",
    }
}

/// Format an `Rgba<u8>` as `#rrggbb` (the alpha is dropped because the form
/// colour pickers don't expose alpha).
fn hex_from_rgba(rgba: Rgba<u8>) -> String {
    let Rgba(parts) = rgba;
    let r = parts.first().copied().unwrap_or(0);
    let g = parts.get(1).copied().unwrap_or(0);
    let b = parts.get(2).copied().unwrap_or(0);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Parse a hex colour string (e.g. `#ff0000`) into an `image::Rgba<u8>`.
fn parse_color(s: &str) -> Result<Rgba<u8>, Error> {
    let parsed = hex_color::HexColor::parse(s)
        .map_err(|e| Error::BadRequest(format!("invalid colour `{s}`: {e}")))?;
    Ok(Rgba(parsed.to_be_bytes()))
}

/// Parsed multipart form for the USB-notecard render endpoint.
struct ParsedRenderForm {
    /// the notecard source: an existing saved notecard id, or a freshly
    /// uploaded one (text + optional name + destination).
    notecard_source: NotecardSource,
    /// the destination for the *render*.
    destination: Destination,
    /// per-side borders in regions: north, south, east, west.
    borders: (u16, u16, u16, u16),
    /// route colour.
    color: Rgba<u8>,
    /// shared output parameters.
    common: CommonParams,
    /// whether to also save the without-route variant.
    with_without_route: bool,
    /// optional GLW overlay. Carried in the multipart form as the
    /// `glw_json` field — a JSON-stringified [`GlwRenderOptions`] — so
    /// the browser doesn't have to send nested objects through
    /// `FormData`.
    glw: Option<GlwRenderOptions>,
    /// free-floating text labels. Carried in the multipart form as the
    /// `labels_json` field — a JSON-stringified `Vec<TextLabel>`.
    labels: Vec<TextLabel>,
    /// logo images. Carried in the multipart form as the `logos_json`
    /// field — a JSON-stringified `Vec<LogoPlacement>`.
    logos: Vec<LogoPlacement>,
    /// user-defined combined slot groups (each a list of slot-anchor names),
    /// carried as the `groups_json` field. Only the placement-slots endpoint
    /// uses them (to report each group's combined free rectangle).
    groups: Vec<Vec<String>>,
}

/// Where the notecard for a render comes from.
enum NotecardSource {
    /// Reuse a saved notecard by id.
    Existing {
        /// the saved notecard's id.
        notecard_id: Uuid,
    },
    /// Use a freshly uploaded notecard; the resolver saves it to the
    /// render's destination before linking, so the
    /// "render and notecard share the same scope" invariant always holds.
    Fresh {
        /// the raw notecard text.
        text: String,
        /// the display name for the saved-notecards row.
        name: String,
    },
}

/// Parse the multipart form for the USB-notecard render endpoint.
async fn parse_render_form(multipart: Multipart) -> Result<ParsedRenderForm, Error> {
    let mut form = crate::usb_notecard::NotecardForm::default();
    let mut color: Rgba<u8> = Rgba([0xff, 0x00, 0x00, 0xff]);
    let mut max_width: Option<u32> = None;
    let mut max_height: Option<u32> = None;
    let mut missing_map_tile_color: Option<Rgba<u8>> = None;
    let mut missing_region_color: Option<Rgba<u8>> = None;
    let mut format = OutputFormat::default();
    let mut with_without_route = false;
    let mut draw_region_rectangles = false;
    let mut draw_region_names = false;
    let mut draw_region_coordinates = false;
    let mut region_label_font_id: Option<String> = None;
    let mut notecard_text: Option<String> = None;
    let mut notecard_file: Option<String> = None;
    let mut notecard_id: Option<Uuid> = None;
    let mut destination_raw: Option<String> = None;
    let mut notecard_name: Option<String> = None;
    let mut glw: Option<GlwRenderOptions> = None;
    let mut labels: Vec<TextLabel> = Vec::new();
    let mut logos: Vec<LogoPlacement> = Vec::new();
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut multipart = multipart;
    while let Some(field) = multipart.next_field().await? {
        let Some(name) = field.name().map(str::to_owned) else {
            continue;
        };
        match name.as_str() {
            "notecard" => {
                let bytes = field.bytes().await?;
                if !bytes.is_empty() {
                    let text = String::from_utf8(bytes.to_vec())
                        .map_err(|e| Error::BadRequest(format!("notecard is not UTF-8: {e}")))?;
                    notecard_file = Some(text);
                }
            }
            "notecard_text" => {
                let text = field.text().await?;
                if !text.trim().is_empty() {
                    notecard_text = Some(text);
                }
            }
            "notecard_id" => {
                let raw = field.text().await?;
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    notecard_id = Some(
                        Uuid::parse_str(trimmed)
                            .map_err(|e| Error::BadRequest(format!("invalid notecard_id: {e}")))?,
                    );
                }
            }
            "border_regions" => form.border_regions = parse_optional_u16(&field.text().await?)?,
            "border_north" => form.border_north = parse_optional_u16(&field.text().await?)?,
            "border_south" => form.border_south = parse_optional_u16(&field.text().await?)?,
            "border_east" => form.border_east = parse_optional_u16(&field.text().await?)?,
            "border_west" => form.border_west = parse_optional_u16(&field.text().await?)?,
            "color" => {
                let raw = field.text().await?;
                color = parse_color(raw.trim())?;
            }
            "max_width" => max_width = Some(parse_u32(&field.text().await?)?),
            "max_height" => max_height = Some(parse_u32(&field.text().await?)?),
            "missing_map_tile_color" => {
                let raw = field.text().await?;
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    missing_map_tile_color = Some(parse_color(trimmed)?);
                }
            }
            "missing_region_color" => {
                let raw = field.text().await?;
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    missing_region_color = Some(parse_color(trimmed)?);
                }
            }
            "format" => {
                let raw = field.text().await?;
                format = match raw.trim().to_ascii_lowercase().as_str() {
                    "png" => OutputFormat::Png,
                    "jpeg" | "jpg" => OutputFormat::Jpeg,
                    other => {
                        return Err(Error::BadRequest(format!("unknown format `{other}`")));
                    }
                };
            }
            "save_without_route" => {
                let raw = field.text().await?;
                with_without_route = parse_form_bool(&raw);
            }
            "draw_region_rectangles" => {
                let raw = field.text().await?;
                draw_region_rectangles = parse_form_bool(&raw);
            }
            "draw_region_names" => {
                let raw = field.text().await?;
                draw_region_names = parse_form_bool(&raw);
            }
            "draw_region_coordinates" => {
                let raw = field.text().await?;
                draw_region_coordinates = parse_form_bool(&raw);
            }
            "region_label_font_id" => {
                let raw = field.text().await?;
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    region_label_font_id = Some(trimmed.to_owned());
                }
            }
            "save_to" => destination_raw = Some(field.text().await?),
            "notecard_name" => {
                let t = field.text().await?;
                if !t.trim().is_empty() {
                    notecard_name = Some(t);
                }
            }
            "glw_json" => {
                let raw = field.text().await?;
                if !raw.trim().is_empty() {
                    glw = Some(
                        serde_json::from_str::<GlwRenderOptions>(&raw)
                            .map_err(|e| Error::BadRequest(format!("invalid glw_json: {e}")))?,
                    );
                }
            }
            "labels_json" => {
                let raw = field.text().await?;
                if !raw.trim().is_empty() {
                    labels = serde_json::from_str::<Vec<TextLabel>>(&raw)
                        .map_err(|e| Error::BadRequest(format!("invalid labels_json: {e}")))?;
                }
            }
            "logos_json" => {
                let raw = field.text().await?;
                if !raw.trim().is_empty() {
                    logos = serde_json::from_str::<Vec<LogoPlacement>>(&raw)
                        .map_err(|e| Error::BadRequest(format!("invalid logos_json: {e}")))?;
                }
            }
            "groups_json" => {
                let raw = field.text().await?;
                if !raw.trim().is_empty() {
                    groups = serde_json::from_str::<Vec<Vec<String>>>(&raw)
                        .map_err(|e| Error::BadRequest(format!("invalid groups_json: {e}")))?;
                }
            }
            _ => {}
        }
    }
    let destination = Destination::parse(destination_raw.as_deref().unwrap_or("personal"))?;
    let max_width = max_width.ok_or_else(|| Error::BadRequest("missing max_width".to_owned()))?;
    let max_height =
        max_height.ok_or_else(|| Error::BadRequest("missing max_height".to_owned()))?;
    validate_dimensions(max_width, max_height)?;
    let borders = form.borders();
    let common = CommonParams {
        max_width,
        max_height,
        missing_map_tile_color,
        missing_region_color,
        format,
        region_overlay: RegionOverlayOptions {
            rectangles: draw_region_rectangles,
            names: draw_region_names,
            coordinates: draw_region_coordinates,
        },
        region_label_font_id,
    };
    let notecard_source = if let Some(id) = notecard_id {
        if notecard_file.is_some() || notecard_text.is_some() {
            return Err(Error::BadRequest(
                "supply either `notecard_id` or notecard text/file, not both".to_owned(),
            ));
        }
        NotecardSource::Existing { notecard_id: id }
    } else {
        let raw = notecard_file.or(notecard_text).ok_or_else(|| {
            Error::BadRequest(
                "supply `notecard_id`, `notecard` file upload, or `notecard_text`".to_owned(),
            )
        })?;
        let name = notecard_name.map_or_else(default_notecard_name, |n| n.trim().to_owned());
        NotecardSource::Fresh { text: raw, name }
    };
    Ok(ParsedRenderForm {
        notecard_source,
        destination,
        borders,
        color,
        common,
        with_without_route,
        glw,
        labels,
        logos,
        groups,
    })
}

/// Compose a fallback display name for an unnamed notecard.
fn default_notecard_name() -> String {
    let now = Utc::now();
    format!("Uploaded {}", now.format("%Y-%m-%d %H:%M:%S UTC"))
}

/// Resolve the notecard source for a USB-notecard render. Returns the parsed
/// `USBNotecard` (for the renderer) and a [`NotecardSummary`] describing
/// the saved row the render will link to. The render's destination is
/// canonical: a reused notecard in a different scope is copied into the
/// render's scope, and a freshly uploaded notecard is always saved into
/// the render's scope. Both behaviours keep the DB-level "render and
/// notecard share the same scope" trigger satisfied without ever
/// surfacing a constraint error to the caller.
async fn resolve_notecard(
    state: &AppState,
    user: &CurrentUser,
    parsed: &ParsedRenderForm,
) -> Result<(USBNotecard, NotecardSummary), Error> {
    match &parsed.notecard_source {
        NotecardSource::Existing { notecard_id } => {
            let row =
                library::assert_can_read_notecard(&state.db, user.user_id, *notecard_id).await?;
            let parsed_notecard: USBNotecard = row.body.parse()?;
            let notecard_scope = library::destination_from_columns(
                row.owner_user_id.clone(),
                row.owner_group_id.clone(),
            )?;
            let effective_id = if notecard_scope == parsed.destination {
                *notecard_id
            } else {
                notecard_routes::insert_notecard_row(
                    state,
                    parsed.destination,
                    user.user_id,
                    &row.name,
                    &row.body,
                )
                .await?
            };
            Ok((
                parsed_notecard,
                NotecardSummary {
                    notecard_id: effective_id,
                    name: row.name,
                    scope: parsed.destination.render_string(),
                },
            ))
        }
        NotecardSource::Fresh { text, name, .. } => {
            let parsed_notecard: USBNotecard = text.parse()?;
            let notecard_id = notecard_routes::insert_notecard_row(
                state,
                parsed.destination,
                user.user_id,
                name,
                text,
            )
            .await?;
            Ok((
                parsed_notecard,
                NotecardSummary {
                    notecard_id,
                    name: name.clone(),
                    scope: parsed.destination.render_string(),
                },
            ))
        }
    }
}

/// Insert a `saved_renders` row with `status='in_progress'` and the supplied
/// settings JSON. `bounds` is `Some` for grid-rectangle renders (the corner
/// coordinates are known at submit time) and `None` for usb-notecard
/// renders, which compute the rectangle in the background and call
/// [`update_render_bounds`] once it is known.
#[expect(
    clippy::too_many_arguments,
    reason = "every argument is a distinct row column; bundling them into a struct would just be noise at the single call sites"
)]
async fn insert_render_row(
    state: &AppState,
    render_id: Uuid,
    destination: Destination,
    created_by: Uuid,
    kind: &str,
    notecard_id: Option<Uuid>,
    settings: &SavedRenderSettings,
    bounds: Option<&GridRectangle>,
) -> Result<(), Error> {
    let (owner_user, owner_group) = match destination {
        Destination::Personal => (Some(created_by.as_bytes().to_vec()), None),
        Destination::Group { group_id } => (None, Some(group_id.as_bytes().to_vec())),
    };
    let now = Utc::now();
    let settings_json = serde_json::to_string(settings)?;
    let (ll_x, ll_y, ur_x, ur_y) = match bounds {
        Some(rect) => (
            Some(i64::from(rect.lower_left_corner().x())),
            Some(i64::from(rect.lower_left_corner().y())),
            Some(i64::from(rect.upper_right_corner().x())),
            Some(i64::from(rect.upper_right_corner().y())),
        ),
        None => (None, None, None, None),
    };
    sqlx::query(
        "INSERT INTO saved_renders \
            (render_id, owner_user_id, owner_group_id, created_by, notecard_id, kind, \
             status, settings_json, created_at, \
             lower_left_x, lower_left_y, upper_right_x, upper_right_y) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_progress', ?7, ?8, ?9, ?10, ?11, ?12)",
    )
    .bind(render_id.as_bytes().to_vec())
    .bind(owner_user)
    .bind(owner_group)
    .bind(created_by.as_bytes().to_vec())
    .bind(notecard_id.map(|id| id.as_bytes().to_vec()))
    .bind(kind)
    .bind(settings_json)
    .bind(now)
    .bind(ll_x)
    .bind(ll_y)
    .bind(ur_x)
    .bind(ur_y)
    .execute(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("insert saved_renders failed: {err}");
        Error::Database
    })?;
    Ok(())
}

/// Backfill the bare-route rectangle bounds on a `saved_notecards` row.
/// The bounds are the bounding box of the notecard's waypoints, *without*
/// the border padding a particular render may have applied — so a single
/// notecard rendered with different borders still has stable cached
/// bounds. The columns are overwritten on every render rather than
/// checked-then-written: the bounding rectangle of a given notecard body
/// is fixed, so the only way the value differs across runs is if region-
/// name resolution returned a different answer (in which case the most
/// recent one is the most useful).
async fn update_notecard_bounds(
    db: &sqlx::SqlitePool,
    notecard_id: Uuid,
    rect: &GridRectangle,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE saved_notecards \
         SET lower_left_x = ?1, lower_left_y = ?2, \
             upper_right_x = ?3, upper_right_y = ?4 \
         WHERE notecard_id = ?5",
    )
    .bind(i64::from(rect.lower_left_corner().x()))
    .bind(i64::from(rect.lower_left_corner().y()))
    .bind(i64::from(rect.upper_right_corner().x()))
    .bind(i64::from(rect.upper_right_corner().y()))
    .bind(notecard_id.as_bytes().to_vec())
    .execute(db)
    .await
    .map_err(|err| {
        tracing::error!("update saved_notecards bounds failed: {err}");
        Error::Database
    })?;
    Ok(())
}

/// Backfill the rectangle bounds on a `saved_renders` row. Used by the
/// usb-notecard path: the rect is computed inside the background job once
/// the notecard has been parsed and any region names resolved, at which
/// point we update the row so the library UI can show the bounds even
/// while the render is still `in_progress`.
async fn update_render_bounds(
    db: &sqlx::SqlitePool,
    render_id: Uuid,
    rect: &GridRectangle,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE saved_renders \
         SET lower_left_x = ?1, lower_left_y = ?2, \
             upper_right_x = ?3, upper_right_y = ?4 \
         WHERE render_id = ?5",
    )
    .bind(i64::from(rect.lower_left_corner().x()))
    .bind(i64::from(rect.lower_left_corner().y()))
    .bind(i64::from(rect.upper_right_corner().x()))
    .bind(i64::from(rect.upper_right_corner().y()))
    .bind(render_id.as_bytes().to_vec())
    .execute(db)
    .await
    .map_err(|err| {
        tracing::error!("update saved_renders bounds failed: {err}");
        Error::Database
    })?;
    Ok(())
}

/// Update a `saved_renders` row to its terminal state. On `Ok`, writes the
/// image files to disk and updates metadata + filenames. On `Err`, records
/// the error message.
async fn finalize_render_row(state: &AppState, render_id: Uuid, outcome: &JobOutcome) {
    let now = Utc::now();
    match outcome {
        JobOutcome::Ok {
            image,
            image_without_route,
            content_type,
            metadata,
            glw_data_id,
        } => {
            let ext = storage::ext_for_content_type(content_type);
            let image_filename = match storage::write_render_file(
                &state.config.storage_dir,
                render_id,
                storage::IMAGE_SUFFIX,
                ext,
                image.clone(),
            )
            .await
            {
                Ok(f) => Some(f),
                Err(err) => {
                    tracing::error!("write primary render image failed: {err}");
                    update_failed(state, render_id, &format!("write image failed: {err}"), now)
                        .await;
                    return;
                }
            };
            let without_filename = if let Some(bytes) = image_without_route {
                match storage::write_render_file(
                    &state.config.storage_dir,
                    render_id,
                    storage::IMAGE_WITHOUT_ROUTE_SUFFIX,
                    ext,
                    bytes.clone(),
                )
                .await
                {
                    Ok(f) => Some(f),
                    Err(err) => {
                        tracing::warn!("write without-route render image failed: {err}");
                        None
                    }
                }
            } else {
                None
            };
            let metadata_json = match serde_json::to_string(metadata) {
                Ok(s) => s,
                Err(err) => {
                    tracing::error!("serialize metadata failed: {err}");
                    update_failed(
                        state,
                        render_id,
                        &format!("serialize metadata failed: {err}"),
                        now,
                    )
                    .await;
                    return;
                }
            };
            // If a GLW overlay was drawn, the worker has both an id to
            // link onto saved_renders and a freshly-stable `SavedId`
            // settings_json to record. Compute the rewritten settings
            // here (close to the DB write) so the worker stays focused
            // on rendering.
            let rewritten_settings_json = match glw_data_id {
                Some(id) => match rewrite_settings_json_with_saved_glw(state, render_id, *id).await
                {
                    Ok(updated) => updated,
                    Err(err) => {
                        tracing::error!("could not rewrite settings_json with glw_data_id: {err}");
                        None
                    }
                },
                None => None,
            };
            let result = sqlx::query(
                "UPDATE saved_renders SET status = 'done', finished_at = ?1, \
                    metadata_json = ?2, content_type = ?3, \
                    image_filename = ?4, image_without_route_filename = ?5, \
                    glw_data_id = COALESCE(?7, glw_data_id), \
                    settings_json = COALESCE(?8, settings_json) \
                 WHERE render_id = ?6",
            )
            .bind(now)
            .bind(metadata_json)
            .bind(*content_type)
            .bind(image_filename)
            .bind(without_filename)
            .bind(render_id.as_bytes().to_vec())
            .bind(glw_data_id.map(|id| id.as_bytes().to_vec()))
            .bind(rewritten_settings_json)
            .execute(&state.db)
            .await;
            if let Err(err) = result {
                tracing::error!("update saved_renders to done failed: {err}");
                state.library_cleanup_dirty.store(true, Ordering::Release);
            }
        }
        JobOutcome::Err(msg) => {
            update_failed(state, render_id, msg, now).await;
        }
    }
}

/// Update a render row to `failed` with the supplied error message.
async fn update_failed(
    state: &AppState,
    render_id: Uuid,
    message: &str,
    now: chrono::DateTime<Utc>,
) {
    if let Err(err) = sqlx::query(
        "UPDATE saved_renders SET status = 'failed', finished_at = ?1, error_message = ?2 \
         WHERE render_id = ?3",
    )
    .bind(now)
    .bind(message)
    .bind(render_id.as_bytes().to_vec())
    .execute(&state.db)
    .await
    {
        tracing::error!("update saved_renders to failed failed: {err}");
    }
}

/// Spawn the background task that renders a grid rectangle.
#[expect(
    clippy::too_many_arguments,
    reason = "this is a one-shot helper that wires together every form field"
)]
fn spawn_grid_rectangle_job(
    state: AppState,
    job_id: JobId,
    job: Arc<JobState>,
    rect: GridRectangle,
    common: CommonParams,
    glw_ctx: Option<GlwJobCtx>,
    labels: Vec<TextLabel>,
    logos: Vec<LogoPlacement>,
) {
    drop(tokio::spawn(async move {
        let result = run_grid_rectangle_job(
            state.clone(),
            Arc::clone(&job),
            rect,
            common,
            glw_ctx,
            labels,
            logos,
        )
        .await;
        let outcome = finish_job(&job, result).await;
        finalize_render_row(&state, job_id, &outcome).await;
        tracing::info!("render job {job_id} finished");
    }));
}

/// Spawn the background task that renders from a USB notecard.
#[expect(
    clippy::too_many_arguments,
    reason = "this is a one-shot helper that wires together every form field"
)]
fn spawn_usb_notecard_job(
    state: AppState,
    job_id: JobId,
    job: Arc<JobState>,
    notecard_id: Uuid,
    notecard: USBNotecard,
    borders: (u16, u16, u16, u16),
    route_color: Rgba<u8>,
    common: CommonParams,
    with_without_route: bool,
    glw_ctx: Option<GlwJobCtx>,
    labels: Vec<TextLabel>,
    logos: Vec<LogoPlacement>,
) {
    drop(tokio::spawn(async move {
        let result = run_usb_notecard_job(
            state.clone(),
            Arc::clone(&job),
            job_id,
            notecard_id,
            notecard,
            borders,
            route_color,
            common,
            with_without_route,
            glw_ctx,
            labels,
            logos,
        )
        .await;
        let outcome = finish_job(&job, result).await;
        finalize_render_row(&state, job_id, &outcome).await;
        tracing::info!("render job {job_id} finished");
    }));
}

// =====================================================================
// Per-region annotation overlay — rectangles, names, grid coordinates.
// Shared between the two render workers and the preview endpoint.
// =====================================================================

/// Reject a render up front when a per-region text overlay (names or
/// coordinates) is requested but no usable font is available, so the submit
/// endpoints fail with a clear `400` instead of the background job dying. A
/// no-text request (rectangles only, or nothing) needs no font and always
/// passes.
fn assert_region_overlay_font(
    state: &AppState,
    opts: RegionOverlayOptions,
    font_id: Option<&str>,
) -> Result<(), Error> {
    if !opts.any_text() {
        return Ok(());
    }
    let font_id = font_id.ok_or_else(|| {
        Error::BadRequest("select a font for the region name / coordinate overlay".to_owned())
    })?;
    if state.fonts.path_for(font_id).is_none() {
        return Err(Error::BadRequest(format!(
            "unknown region label font_id `{font_id}`"
        )));
    }
    Ok(())
}

/// Whether the per-region name / coordinate text overlay should be drawn for a
/// render whose regions render at `pixels_per_region` pixels each and which
/// covers `region_count` regions. Both gates must pass: the text only fits
/// (and only reads) above a minimum per-region size, and resolving a name per
/// region only stays affordable below a maximum region count.
const fn region_text_overlay_allowed(pixels_per_region: f32, region_count: usize) -> bool {
    pixels_per_region >= MIN_PIXELS_PER_REGION_FOR_REGION_LABELS
        && region_count <= MAX_REGIONS_FOR_REGION_LABELS
}

/// Draw the requested per-region annotations onto `map`.
///
/// The rectangle outline is cheap and always drawn when requested. The name
/// and coordinate text is gated twice — both must hold or the text is skipped
/// (rectangles still draw): each region must render at least
/// [`MIN_PIXELS_PER_REGION_FOR_REGION_LABELS`] pixels (otherwise the text does
/// not fit), and the render must cover at most [`MAX_REGIONS_FOR_REGION_LABELS`]
/// regions (otherwise resolving every region name fans out into too many
/// upstream lookups). When both name and coordinate overlays are on, the
/// coordinates are stacked above the name.
async fn apply_region_overlay(
    state: &AppState,
    opts: RegionOverlayOptions,
    font_id: Option<&str>,
    missing_region_color: Option<Rgba<u8>>,
    map: &mut Map,
    progress: Option<&tokio::sync::mpsc::Sender<MapProgressEvent>>,
) -> Result<(), Error> {
    use sl_map_apis::map_tiles::MapLike as _;
    use sl_types::map::GridRectangleLike as _;
    if !opts.any() {
        return Ok(());
    }
    // The text overlays (names / coordinates) are gated twice — too-small regions
    // can't hold the text, and too-many regions would fan name resolution out
    // into thousands of lookups. When the gate fails the per-region loop is
    // skipped entirely; rectangles (which need no loop) are still drawn below.
    let pixels_per_region = map.pixels_per_region();
    let region_count = usize::from(map.size_x()).saturating_mul(usize::from(map.size_y()));
    let run_loop = opts.any_text() && region_text_overlay_allowed(pixels_per_region, region_count);
    if !run_loop {
        if opts.any_text() {
            tracing::info!(
                pixels_per_region,
                region_count,
                "skipping region name/coordinate overlay (regions too small or too many)"
            );
        }
        // No per-region loop, so draw the outlines on their own.
        if opts.rectangles {
            draw_region_rectangles(map);
        }
        return Ok(());
    }
    let font_id = font_id.ok_or_else(|| {
        Error::BadRequest("select a font for the region name / coordinate overlay".to_owned())
    })?;
    let font_path = state
        .fonts
        .path_for(font_id)
        .ok_or_else(|| Error::BadRequest(format!("unknown region label font_id `{font_id}`")))?;
    let font = sl_map_apis::text::load_font(font_path)?;
    let scale = ab_glyph::PxScale::from(
        (pixels_per_region * REGION_LABEL_FONT_FACTOR)
            .clamp(REGION_LABEL_FONT_MIN_PX, REGION_LABEL_FONT_MAX_PX),
    );
    let style = sl_map_apis::text::LabelStyle {
        scale,
        fg: Rgba([255, 255, 255, 255]),
        shadow: Rgba([0, 0, 0, 180]),
        align: sl_map_apis::coverage::HAlign::Left,
    };
    // Resolving a name is a (cached but possibly cold) upstream lookup per
    // region, so report it as progress — but only when names are actually being
    // looked up (coordinates / rectangles are computed locally and need none).
    let total_regions = u32::try_from(region_count).unwrap_or(u32::MAX);
    if let Some(tx) = progress.filter(|_| opts.names) {
        drop(tx.try_send(MapProgressEvent::RegionNamesPlanned { total_regions }));
    }
    let mut resolved: u32 = 0;
    for x in map.x_range() {
        for y in map.y_range() {
            let grid = GridCoordinates::new(x, y);
            let Some((left, top, width, height)) = region_pixel_rect(map, &grid) else {
                continue;
            };
            // Resolve the name (when enabled). The result also tells us whether
            // the region exists: `Ok(None)` is a confirmed-missing region, which
            // we paint with the missing-region colour when that is enabled.
            // `Err` is an inconclusive lookup — leave it untouched.
            let mut region_name: Option<String> = None;
            let mut missing = false;
            if opts.names {
                let lookup = {
                    let mut region = state.region_cache.lock().await;
                    region.get_region_name(&grid).await
                };
                match lookup {
                    Ok(Some(name)) => region_name = Some(name.to_string()),
                    Ok(None) => missing = true,
                    Err(err) => {
                        tracing::debug!("region name lookup failed for {grid:?}: {err}");
                    }
                }
                if let Some(tx) = progress {
                    drop(tx.try_send(MapProgressEvent::RegionNameResolved {
                        index: resolved,
                        total: total_regions,
                    }));
                }
                resolved = resolved.saturating_add(1);
            }
            // Layer per region, bottom-up: missing-region fill, then the outline,
            // then the text — so each sits above the previous.
            if let Some(color) = missing_region_color.filter(|_| missing) {
                map.draw_filled_rect(left, top, width, height, color);
            }
            if opts.rectangles {
                map.draw_hollow_rect(left, top, width, height, REGION_RECTANGLE_COLOR);
            }
            let mut lines: Vec<String> = Vec::new();
            if opts.coordinates {
                lines.push(format!("({x}, {y})"));
            }
            if let Some(name) = region_name {
                lines.push(name);
            }
            if lines.is_empty() {
                continue;
            }
            let (_text_w, text_h) = sl_map_apis::text::measure_text(scale, &font, &lines);
            // SL y points up while image y points down, so the region's bottom
            // edge (the text anchor) is `top + height`.
            let bottom = top.saturating_add(height);
            let origin_x = i32::try_from(left)
                .unwrap_or(0)
                .saturating_add(REGION_LABEL_PADDING);
            let origin_y = i32::try_from(bottom)
                .unwrap_or(0)
                .saturating_sub(REGION_LABEL_PADDING)
                .saturating_sub(i32::try_from(text_h).unwrap_or(0));
            map.draw_text_label((origin_x, origin_y), &lines, &style, &font);
        }
    }
    Ok(())
}

/// The pixel rectangle `(left, top, width, height)` a region occupies in `map`,
/// or `None` if the region is outside it. The two opposite corners are mapped to
/// pixels (same pattern as `crop_imm_grid_rectangle`) so no floating-point
/// pixel-per-region rounding is needed.
fn region_pixel_rect(map: &Map, grid: &GridCoordinates) -> Option<(u32, u32, u32, u32)> {
    use sl_map_apis::map_tiles::MapLike as _;
    let (x0, y0) =
        map.pixel_coordinates_for_coordinates(grid, &RegionCoordinates::new(0f32, 0f32, 0f32))?;
    let (x1, y1) =
        map.pixel_coordinates_for_coordinates(grid, &RegionCoordinates::new(256f32, 256f32, 0f32))?;
    Some((x0.min(x1), y0.min(y1), x0.abs_diff(x1), y0.abs_diff(y1)))
}

/// Draw a hairline outline around every region of `map`.
fn draw_region_rectangles(map: &mut Map) {
    use sl_map_apis::map_tiles::MapLike as _;
    use sl_types::map::GridRectangleLike as _;
    for x in map.x_range() {
        for y in map.y_range() {
            let grid = GridCoordinates::new(x, y);
            if let Some((left, top, width, height)) = region_pixel_rect(map, &grid) {
                map.draw_hollow_rect(left, top, width, height, REGION_RECTANGLE_COLOR);
            }
        }
    }
}

/// Run the grid-rectangle render to completion.
async fn run_grid_rectangle_job(
    state: AppState,
    job: Arc<JobState>,
    rect: GridRectangle,
    common: CommonParams,
    glw_ctx: Option<GlwJobCtx>,
    labels: Vec<TextLabel>,
    logos: Vec<LogoPlacement>,
) -> Result<JobOutcome, Error> {
    let metadata = build_metadata(&rect);
    // Kept for the overlay-only occupancy map below; `new_with_progress` moves
    // `rect` into the tiled map.
    let occ_rect = rect.clone();
    let (tx, rx) = tokio::sync::mpsc::channel::<MapProgressEvent>(256);
    let forwarder = tokio::spawn(forward_events(Arc::clone(&job), rx));
    let mut map = {
        let mut cache = state.map_tile_cache.lock().await;
        Map::new_with_progress(
            &mut cache,
            common.max_width,
            common.max_height,
            rect,
            common.missing_map_tile_color,
            common.missing_region_color,
            Some(&tx),
        )
        .await?
    };
    // Layering: base map below, then the per-region annotation overlay, then
    // the GLW overlay. No route for the grid-rectangle path. Labels go last,
    // above everything. The overlay resolves region names (when enabled), so it
    // keeps reporting progress on `tx` — drop the sender and join the forwarder
    // only after it returns.
    apply_region_overlay(
        &state,
        common.region_overlay,
        common.region_label_font_id.as_deref(),
        // The final render fills missing regions properly during tile building
        // (`Map::new_with_progress`), so the overlay never paints them here.
        None,
        &mut map,
        Some(&tx),
    )
    .await?;
    drop(tx);
    // wait for the forwarder so the event history is complete before we
    // signal completion to subscribers
    let _join = forwarder.await;
    let glw_data_id = apply_glw_overlay_to_map(&state, glw_ctx.as_ref(), &mut map).await?;
    // Free space for labels/logos is measured on an overlay-only map (GLW shapes
    // only, no route on the grid path); the same planning rejected an over-full
    // slot at submit time, so this normally cannot fail here.
    let (label_draws, logo_draws) = plan_placements(
        &state,
        occ_rect,
        &common,
        glw_ctx.as_ref(),
        None,
        &labels,
        &logos,
    )
    .await?;
    execute_labels(&label_draws, &mut map);
    execute_logos(&logo_draws, &mut map);
    let image = encode_map(&map, common.format)?;
    Ok(JobOutcome::Ok {
        image,
        image_without_route: None,
        content_type: common.format.content_type(),
        metadata,
        glw_data_id,
    })
}

/// Run the USB-notecard render to completion.
#[expect(
    clippy::too_many_arguments,
    reason = "this is a one-shot helper invoked from a single spawn site"
)]
async fn run_usb_notecard_job(
    state: AppState,
    job: Arc<JobState>,
    render_id: Uuid,
    notecard_id: Uuid,
    notecard: USBNotecard,
    borders: (u16, u16, u16, u16),
    route_color: Rgba<u8>,
    common: CommonParams,
    with_without_route: bool,
    glw_ctx: Option<GlwJobCtx>,
    labels: Vec<TextLabel>,
    logos: Vec<LogoPlacement>,
) -> Result<JobOutcome, Error> {
    let (bare_rect, rect) = resolve_notecard_rect(&state, &notecard, borders).await?;
    // Cache the bare rectangle (without border padding) on the notecard
    // row so the library UI can show its bounds without redoing region
    // resolution. The bare box keeps notecard bounds == route's bounding box.
    update_notecard_bounds(&state.db, notecard_id, &bare_rect).await?;
    // Backfill the bounds on the saved_renders row now that we know the
    // rectangle; the library UI shows them even for `in_progress` rows.
    update_render_bounds(&state.db, render_id, &rect).await?;
    let metadata = build_metadata(&rect);
    // Kept for the overlay-only occupancy map below; `new_with_progress` moves
    // `rect` into the tiled map.
    let occ_rect = rect.clone();
    let (tx, rx) = tokio::sync::mpsc::channel::<MapProgressEvent>(256);
    let forwarder = tokio::spawn(forward_events(Arc::clone(&job), rx));
    let (image_without_route, map, glw_data_id) = {
        let mut map = {
            let mut cache = state.map_tile_cache.lock().await;
            Map::new_with_progress(
                &mut cache,
                common.max_width,
                common.max_height,
                rect,
                common.missing_map_tile_color,
                common.missing_region_color,
                Some(&tx),
            )
            .await?
        };
        // Layering: encode the without-route variant BEFORE any
        // overlays so the diagnostic save shows just the bare map.
        let without_route = if with_without_route {
            Some(encode_map(&map, common.format)?)
        } else {
            None
        };
        // Per-region annotation overlay sits just above the bare tiles (and so
        // is excluded from the without-route diagnostic above), below the GLW
        // overlay and route.
        apply_region_overlay(
            &state,
            common.region_overlay,
            common.region_label_font_id.as_deref(),
            // The final render fills missing regions properly during tile
            // building (`Map::new_with_progress`), so the overlay never paints
            // them here.
            None,
            &mut map,
            Some(&tx),
        )
        .await?;
        // GLW overlay sits between the base map and the route, so the
        // route line stays the most-readable element of the final
        // image.
        let glw_data_id = apply_glw_overlay_to_map(&state, glw_ctx.as_ref(), &mut map).await?;
        {
            let mut region = state.region_cache.lock().await;
            map.draw_route_with_progress(&mut region, &notecard, route_color, Some(&tx))
                .await?;
        }
        // Labels and logos go last, above the route, sharing one
        // mutually-exclusive pool of placement slots. Their free space is
        // measured on an overlay-only map (route + GLW shapes, legend excluded)
        // rather than the opaque tiled map; the same planning rejected an
        // over-full slot at submit time, so this normally cannot fail here.
        let (label_draws, logo_draws) = plan_placements(
            &state,
            occ_rect,
            &common,
            glw_ctx.as_ref(),
            Some((&notecard, route_color)),
            &labels,
            &logos,
        )
        .await?;
        execute_labels(&label_draws, &mut map);
        execute_logos(&logo_draws, &mut map);
        (without_route, map, glw_data_id)
    };
    drop(tx);
    let _join = forwarder.await;
    let image = encode_map(&map, common.format)?;
    Ok(JobOutcome::Ok {
        image,
        image_without_route,
        content_type: common.format.content_type(),
        metadata,
        glw_data_id,
    })
}

/// Forward `MapProgressEvent`s coming from the renderer into the job's
/// recorded event history, converting them to `ProgressDto` on the way.
async fn forward_events(job: Arc<JobState>, mut rx: tokio::sync::mpsc::Receiver<MapProgressEvent>) {
    while let Some(event) = rx.recv().await {
        record_event(&job, ProgressDto::from(event)).await;
    }
}

/// Compute the metadata block (aspect ratio + PPS HUD config) for a
/// rendered grid rectangle.
fn build_metadata(rect: &GridRectangle) -> Metadata {
    let aspect_x = rect.size_x();
    let aspect_y = rect.size_y();
    let aspect_ratio = f32::from(aspect_x) / f32::from(aspect_y);
    Metadata {
        aspect_x,
        aspect_y,
        aspect_ratio,
        pps_hud_config: rect.pps_hud_config(),
    }
}

/// Encode the rendered map as image bytes in the requested format.
fn encode_map(map: &Map, format: OutputFormat) -> Result<Bytes, Error> {
    let mut buf: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    sl_map_apis::map_tiles::MapLike::image(map).write_to(&mut cursor, format.image_format())?;
    Ok(Bytes::from(buf))
}

/// Finalise the job: record either Ok or Err and publish via the watch
/// channel so SSE handlers can emit the final `done` / `error` event.
/// Returns the [`JobOutcome`] for the persistence step.
async fn finish_job(job: &Arc<JobState>, result: Result<JobOutcome, Error>) -> JobOutcome {
    let outcome = match result {
        Ok(o) => o,
        Err(e) => {
            let message = format!("{e}");
            record_event(
                job,
                ProgressDto::Error {
                    message: message.clone(),
                },
            )
            .await;
            JobOutcome::Err(message)
        }
    };
    if matches!(outcome, JobOutcome::Ok { .. }) {
        record_event(job, ProgressDto::Done).await;
    }
    let arc = Arc::new(outcome.clone());
    drop(job.outcome.send_replace(Some(arc)));
    outcome
}

/// Reject `max_width` / `max_height` outside the per-side caps. Both must
/// be > 0 and <= [`MAX_OUTPUT_DIMENSION`].
fn validate_dimensions(max_width: u32, max_height: u32) -> Result<(), Error> {
    if max_width == 0 || max_height == 0 {
        return Err(Error::BadRequest(
            "max_width and max_height must be greater than zero".to_owned(),
        ));
    }
    if max_width > MAX_OUTPUT_DIMENSION || max_height > MAX_OUTPUT_DIMENSION {
        return Err(Error::BadRequest(format!(
            "max_width and max_height must each be <= {MAX_OUTPUT_DIMENSION}"
        )));
    }
    // The product is the real allocation bound; the per-side cap alone would
    // still allow a ~4 GiB buffer. u64 so the multiply cannot overflow.
    if u64::from(max_width).saturating_mul(u64::from(max_height)) > MAX_OUTPUT_AREA {
        return Err(Error::BadRequest(format!(
            "max_width * max_height must be <= {MAX_OUTPUT_AREA} pixels"
        )));
    }
    Ok(())
}

/// Reject the request if the user already has
/// [`MAX_CONCURRENT_RENDERS_PER_USER`] or more renders in progress. The
/// count is not strictly atomic with the subsequent insert — two requests
/// arriving in the same millisecond could both pass — but the race window
/// is small and the cap exists to limit accidental DoS rather than to be a
/// hard quota.
async fn assert_under_concurrent_limit(db: &sqlx::SqlitePool, user_id: Uuid) -> Result<(), Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM saved_renders \
         WHERE created_by = ?1 AND status = 'in_progress'",
    )
    .bind(user_id.as_bytes().to_vec())
    .fetch_one(db)
    .await
    .map_err(|err| {
        tracing::error!("count in-progress renders failed: {err}");
        Error::Database
    })?;
    if count >= MAX_CONCURRENT_RENDERS_PER_USER {
        return Err(Error::Forbidden(format!(
            "at most {MAX_CONCURRENT_RENDERS_PER_USER} renders may be in progress per user; \
             wait for one to finish"
        )));
    }
    Ok(())
}

/// Parse a possibly-empty u16 from a form field.
fn parse_optional_u16(s: &str) -> Result<Option<u16>, Error> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<u16>()
        .map(Some)
        .map_err(|e| Error::BadRequest(format!("invalid u16 `{trimmed}`: {e}")))
}

/// Parse a required u32 from a form field.
fn parse_u32(s: &str) -> Result<u32, Error> {
    s.trim()
        .parse::<u32>()
        .map_err(|e| Error::BadRequest(format!("invalid u32 `{s}`: {e}")))
}

/// Interpret a multipart-form checkbox value as a boolean. Treats the usual
/// truthy spellings as `true` and everything else (including absence, handled
/// by the field never appearing) as `false`.
fn parse_form_bool(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "on" | "true" | "yes"
    )
}

// =====================================================================
// GLW overlay plumbing — shared between the two render workers.
// =====================================================================

/// Inputs the render worker needs to resolve and apply a GLW overlay.
struct GlwJobCtx {
    /// the user's GLW request payload as it arrived from the form.
    options: GlwRenderOptions,
    /// destination the render lives in (and the auto-saved GLW row
    /// will inherit).
    destination: Destination,
    /// avatar id to record as the GLW row's `created_by`.
    created_by: Uuid,
}

/// Resolved GLW event ready to draw onto the map, paired with the
/// `saved_glw_data` row id the render should reference.
struct ResolvedGlwEvent {
    /// the deserialised event.
    event: sl_glw::GlwEvent,
    /// the saved_glw_data row this event lives in.
    glw_data_id: Uuid,
}

/// Resolve the GLW source to an event plus the saved_glw_data row id.
/// For fresh-fetch sources (event id / key / pasted JSON) this inserts
/// a new `saved_glw_data` row. For the `SavedId` source it just reads
/// the existing row.
async fn resolve_glw_event(
    state: &AppState,
    ctx: &GlwJobCtx,
) -> Result<Option<ResolvedGlwEvent>, Error> {
    match &ctx.options.source {
        GlwSource::SavedId { glw_data_id } => {
            let row =
                crate::library::assert_can_read_glw_data(&state.db, ctx.created_by, *glw_data_id)
                    .await?;
            let event: sl_glw::GlwEvent = serde_json::from_str(&row.payload_json)?;
            Ok(Some(ResolvedGlwEvent {
                event,
                glw_data_id: *glw_data_id,
            }))
        }
        GlwSource::EventId { event_id } => {
            let id = sl_glw::EventId::new(*event_id);
            let event_opt = {
                let mut cache = state.glw_event_cache.lock().await;
                cache.get_event_by_id(id).await?
            };
            let Some(event) = event_opt else {
                return Ok(None);
            };
            let now = Utc::now();
            let name = default_or_user_name(
                ctx.options.save_as.as_deref(),
                &format!(
                    "Event {event_id} fetched {ts}",
                    ts = now.format("%Y-%m-%d %H:%M UTC")
                ),
            );
            let payload_json = serde_json::to_string(&event)?;
            let glw_data_id = crate::routes::glw::insert_glw_data_row(
                state,
                &crate::routes::glw::InsertGlwData {
                    destination: ctx.destination,
                    created_by: ctx.created_by,
                    name: &name,
                    source_kind: crate::library::GlwDataSourceKind::EventId,
                    source_event_id: Some(*event_id),
                    source_event_key: None,
                    payload_json: &payload_json,
                    event_id: Some(event.event_id.get()),
                    event_key: Some(event.event_key.as_str()),
                    event_name: Some(event.event_name.as_str()),
                    fetched_at: now,
                },
            )
            .await?;
            Ok(Some(ResolvedGlwEvent { event, glw_data_id }))
        }
        GlwSource::EventKey { event_key } => {
            let key = sl_glw::GlwEventKey::new(event_key);
            let event_opt = {
                let mut cache = state.glw_event_cache.lock().await;
                cache.get_event_by_key(&key).await?
            };
            let Some(event) = event_opt else {
                return Ok(None);
            };
            let now = Utc::now();
            let name = default_or_user_name(
                ctx.options.save_as.as_deref(),
                &format!(
                    "Key \"{event_key}\" fetched {ts}",
                    ts = now.format("%Y-%m-%d %H:%M UTC")
                ),
            );
            let payload_json = serde_json::to_string(&event)?;
            let glw_data_id = crate::routes::glw::insert_glw_data_row(
                state,
                &crate::routes::glw::InsertGlwData {
                    destination: ctx.destination,
                    created_by: ctx.created_by,
                    name: &name,
                    source_kind: crate::library::GlwDataSourceKind::EventKey,
                    source_event_id: None,
                    source_event_key: Some(event_key.as_str()),
                    payload_json: &payload_json,
                    event_id: Some(event.event_id.get()),
                    event_key: Some(event.event_key.as_str()),
                    event_name: Some(event.event_name.as_str()),
                    fetched_at: now,
                },
            )
            .await?;
            Ok(Some(ResolvedGlwEvent { event, glw_data_id }))
        }
        GlwSource::PastedJson { payload } => {
            let event: sl_glw::GlwEvent = serde_json::from_str(payload)?;
            let now = Utc::now();
            let name = default_or_user_name(
                ctx.options.save_as.as_deref(),
                &format!("Pasted JSON {}", now.format("%Y-%m-%d %H:%M UTC")),
            );
            let payload_json = serde_json::to_string(&event)?;
            let glw_data_id = crate::routes::glw::insert_glw_data_row(
                state,
                &crate::routes::glw::InsertGlwData {
                    destination: ctx.destination,
                    created_by: ctx.created_by,
                    name: &name,
                    source_kind: crate::library::GlwDataSourceKind::PastedJson,
                    source_event_id: None,
                    source_event_key: None,
                    payload_json: &payload_json,
                    event_id: Some(event.event_id.get()),
                    event_key: Some(event.event_key.as_str()),
                    event_name: Some(event.event_name.as_str()),
                    fetched_at: now,
                },
            )
            .await?;
            Ok(Some(ResolvedGlwEvent { event, glw_data_id }))
        }
    }
}

/// If the user supplied a non-empty name, return it trimmed; otherwise
/// return the generated default.
fn default_or_user_name(supplied: Option<&str>, default: &str) -> String {
    let trimmed = supplied.map(str::trim).filter(|s| !s.is_empty());
    match trimmed {
        Some(s) => s.to_owned(),
        None => default.to_owned(),
    }
}

/// Apply a GLW overlay onto `map` if `ctx` is `Some`. Returns the id
/// of the `saved_glw_data` row the worker should reference on
/// `saved_renders.glw_data_id`, or `None` if no overlay was drawn
/// (no GLW requested, or the server returned no event).
async fn apply_glw_overlay_to_map(
    state: &AppState,
    ctx: Option<&GlwJobCtx>,
    map: &mut Map,
) -> Result<Option<Uuid>, Error> {
    use sl_glw::MapLikeGlwExt as _;
    let Some(ctx) = ctx else {
        return Ok(None);
    };
    // Resolve the font first so a missing-font request fails fast,
    // before any HTTP fetch or DB writes.
    let font_path = state
        .fonts
        .path_for(&ctx.options.font_id)
        .ok_or_else(|| Error::BadRequest(format!("unknown font_id `{}`", ctx.options.font_id)))?;
    let font = sl_map_apis::text::load_font(font_path)?;

    let resolved = resolve_glw_event(state, ctx).await?;
    let Some(resolved) = resolved else {
        tracing::warn!("GLW event not found (server returned no event); rendering without overlay");
        return Ok(None);
    };
    let style = build_glw_style(&ctx.options.style, ctx.options.legend_slot.as_deref())?;
    map.draw_glw_event_with_font(&resolved.event, &style, &font)?;
    Ok(Some(resolved.glw_data_id))
}

/// Start from [`sl_glw::GlwStyle::default`] with the legend placed in the
/// requested slot (defaulting to `TopLeft`), then layer the user's
/// per-element colour and toggle overrides.
fn build_glw_style(
    overrides: &GlwStyleOverrides,
    legend_slot: Option<&str>,
) -> Result<sl_glw::GlwStyle, Error> {
    let mut style = sl_glw::GlwStyle {
        legend_position: legend_position_from_slot(legend_slot)?,
        draw_margin_band: overrides.margin_band,
        ..sl_glw::GlwStyle::default()
    };
    if let Some(c) = overrides.area_outline_color.as_deref() {
        style.palette.area_outline = parse_color(c.trim())?;
    }
    if let Some(c) = overrides.circle_outline_color.as_deref() {
        style.palette.circle_outline = parse_color(c.trim())?;
    }
    if let Some(c) = overrides.margin_outline_color.as_deref() {
        style.palette.margin_outline = parse_color(c.trim())?;
    }
    if let Some(c) = overrides.wind_color.as_deref() {
        style.palette.wind_arrow = parse_color(c.trim())?;
    }
    if let Some(c) = overrides.current_color.as_deref() {
        style.palette.current_arrow = parse_color(c.trim())?;
    }
    if let Some(c) = overrides.wave_color.as_deref() {
        style.palette.wave_glyph = parse_color(c.trim())?;
    }
    if let Some(c) = overrides.label_color.as_deref() {
        style.palette.label_fg = parse_color(c.trim())?;
    }
    Ok(style)
}

/// Map a legend-slot name to a placement slot (or `None` to hide it). Absent /
/// empty → `TopLeft` (back-compat); `"none"` → hidden; any of the nine slot
/// names → that slot; anything else is a bad request.
fn legend_position_from_slot(
    slot: Option<&str>,
) -> Result<Option<sl_map_apis::coverage::PlacementSlot>, Error> {
    use sl_map_apis::coverage::PlacementSlot;
    let name = match slot {
        None => return Ok(Some(PlacementSlot::TopLeft)),
        Some(s) => s.trim(),
    };
    match name {
        "" => Ok(Some(PlacementSlot::TopLeft)),
        "none" => Ok(None),
        other => other
            .parse::<PlacementSlot>()
            .map(Some)
            .map_err(|err| Error::BadRequest(err.to_string())),
    }
}

/// The slot anchor the base legend occupies for this job (if any): `None`
/// when there is no GLW overlay, the legend is hidden, or the slot is
/// invalid (the invalid case fails earlier in `apply_glw_overlay_to_map`).
fn legend_slot_of(ctx: Option<&GlwJobCtx>) -> Option<sl_map_apis::coverage::PlacementSlot> {
    let ctx = ctx?;
    legend_position_from_slot(ctx.options.legend_slot.as_deref())
        .ok()
        .flatten()
}

/// Parse an optional horizontal-alignment name; absent/empty → `None` (use
/// the slot default).
fn parse_h_align(value: Option<&str>) -> Result<Option<sl_map_apis::coverage::HAlign>, Error> {
    use sl_map_apis::coverage::HAlign;
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some("left") => Ok(Some(HAlign::Left)),
        Some("center") => Ok(Some(HAlign::Center)),
        Some("right") => Ok(Some(HAlign::Right)),
        Some(other) => Err(Error::BadRequest(format!("invalid h_align `{other}`"))),
    }
}

/// Parse an optional vertical-alignment name; absent/empty → `None` (use the
/// slot default).
fn parse_v_align(value: Option<&str>) -> Result<Option<sl_map_apis::coverage::VAlign>, Error> {
    use sl_map_apis::coverage::VAlign;
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some("top") => Ok(Some(VAlign::Top)),
        Some("center") => Ok(Some(VAlign::Center)),
        Some("bottom") => Ok(Some(VAlign::Bottom)),
        Some(other) => Err(Error::BadRequest(format!("invalid v_align `{other}`"))),
    }
}

/// Parse a placement's combined slot group from the request: each name in
/// `names` parsed to a [`PlacementSlot`], always including `anchor`. An empty
/// `names` (the common single-slot case, and old saved settings) yields just
/// `[anchor]`.
fn parse_slot_group(
    anchor: sl_map_apis::coverage::PlacementSlot,
    names: &[String],
) -> Result<Vec<sl_map_apis::coverage::PlacementSlot>, Error> {
    if names.is_empty() {
        return Ok(vec![anchor]);
    }
    let mut group: Vec<sl_map_apis::coverage::PlacementSlot> = Vec::with_capacity(names.len());
    for name in names {
        let slot = name
            .parse::<sl_map_apis::coverage::PlacementSlot>()
            .map_err(|err: sl_map_apis::coverage::ParsePlacementSlotError| {
                Error::BadRequest(err.to_string())
            })?;
        if !group.contains(&slot) {
            group.push(slot);
        }
    }
    if !group.contains(&anchor) {
        group.push(anchor);
    }
    Ok(group)
}

/// Resolve a placement to the slots it reserves and the pixel rectangle to fit
/// content into. A single-slot placement uses that slot's own free rectangle;
/// a combined placement (more than one slot in `group`) uses the largest free
/// rectangle within exactly those slots' thirds ([`OccupancyGrid::subset_rect`])
/// and reserves the whole group. Shared by labels and logos.
fn resolve_placement(
    anchor: sl_map_apis::coverage::PlacementSlot,
    group: &[sl_map_apis::coverage::PlacementSlot],
    slots: &[sl_map_apis::coverage::PlacementSlotInfo],
    grid: &sl_map_apis::coverage::OccupancyGrid,
) -> Result<
    (
        Vec<sl_map_apis::coverage::PlacementSlot>,
        sl_map_apis::coverage::PixelRect,
    ),
    Error,
> {
    if group.len() > 1 {
        let rect = grid.subset_rect(group).ok_or_else(|| {
            Error::BadRequest(format!(
                "the combined slot at `{anchor}` is fully covered; no room for content"
            ))
        })?;
        Ok((group.to_vec(), rect))
    } else {
        let info = slots
            .iter()
            .find(|info| info.slot == anchor)
            .ok_or_else(|| Error::BadRequest(format!("slot `{anchor}` not found")))?;
        let rect = info.free_rect.ok_or_else(|| {
            Error::BadRequest(format!(
                "slot `{anchor}` is fully covered; no room for content"
            ))
        })?;
        Ok((vec![anchor], rect))
    }
}

/// Reserve a placement's slots in the shared pool, rejecting any clash with
/// the legend, with already-reserved slots from the *other* placement kind
/// (`others`), or with slots reserved earlier in this pass (`used`). `what`
/// names the placement kind for the error message.
fn reserve(
    reserved: &[sl_map_apis::coverage::PlacementSlot],
    used: &mut Vec<sl_map_apis::coverage::PlacementSlot>,
    others: &[sl_map_apis::coverage::PlacementSlot],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    what: &str,
) -> Result<(), Error> {
    for &slot in reserved {
        if legend_slot == Some(slot) {
            return Err(Error::BadRequest(format!(
                "a {what} uses slot `{slot}` which is occupied by the legend"
            )));
        }
        if used.contains(&slot) || others.contains(&slot) {
            return Err(Error::BadRequest(format!(
                "two placements target the same slot `{slot}`"
            )));
        }
        used.push(slot);
    }
    Ok(())
}

/// One accepted label, ready to draw by [`execute_labels`].
struct LabelDraw {
    /// the text, one entry per line.
    lines: Vec<String>,
    /// the resolved font to render with.
    font: ab_glyph::FontVec,
    /// colour, scale and shadow for the text.
    style: sl_map_apis::text::LabelStyle,
    /// top-left pixel origin within the image.
    origin: (i32, i32),
}

/// One accepted logo (already scaled), ready to composite by
/// [`execute_logos`].
struct LogoDraw {
    /// the decoded (and optionally doubled) logo bitmap.
    img: image::RgbaImage,
    /// x pixel coordinate of the logo's top-left corner.
    x: i64,
    /// y pixel coordinate of the logo's top-left corner.
    y: i64,
}

/// Plan the free-floating text labels against the free space measured on
/// `occupancy` (an overlay-only map carrying just the route + GLW shapes, so
/// the opaque base tiles do not count as covered). Rejects (as a `BadRequest`)
/// any label that overflows its (single-slot or spanned) free space, or whose
/// reserved slots clash with the legend, another label, or a slot already
/// reserved by a logo (`reserved_by_others`). Each label is aligned within its
/// free rectangle using its own alignment, defaulting to the slot's outward
/// alignment. Returns the draw list (to hand to [`execute_labels`]) and the
/// set of slots the labels reserved so logos can avoid them.
fn plan_labels(
    fonts: &crate::fonts::FontDirectory,
    labels: &[TextLabel],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    reserved_by_others: &[sl_map_apis::coverage::PlacementSlot],
    occupancy: &Map,
) -> Result<(Vec<LabelDraw>, Vec<sl_map_apis::coverage::PlacementSlot>), Error> {
    let mut used: Vec<sl_map_apis::coverage::PlacementSlot> = Vec::new();
    let mut draws: Vec<LabelDraw> = Vec::new();
    if labels.is_empty() {
        return Ok((draws, used));
    }
    // Authoritative free space, measured once on the overlay-only map.
    let grid = sl_map_apis::coverage::OccupancyGrid::from_map(
        occupancy,
        sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
    );
    let slots = grid.evaluate_slots();
    // Validate + reserve + measure, collecting one draw item per visible label
    // so nothing is drawn until every label has been accepted.
    for label in labels {
        let lines: Vec<String> = label.lines.clone();
        if lines.iter().all(|line| line.trim().is_empty()) {
            // a blank label draws nothing and reserves nothing
            continue;
        }
        if !(label.font_px.is_finite() && label.font_px > 0f32) {
            return Err(Error::BadRequest(format!(
                "label font size must be a positive number of pixels, got {}",
                label.font_px
            )));
        }
        let anchor: sl_map_apis::coverage::PlacementSlot =
            label
                .slot
                .parse()
                .map_err(|err: sl_map_apis::coverage::ParsePlacementSlotError| {
                    Error::BadRequest(err.to_string())
                })?;
        let group = parse_slot_group(anchor, &label.slots)?;
        let (reserved, rect) = resolve_placement(anchor, &group, &slots, &grid)?;
        reserve(
            &reserved,
            &mut used,
            reserved_by_others,
            legend_slot,
            "label",
        )?;
        let font_path = fonts
            .path_for(&label.font_id)
            .ok_or_else(|| Error::BadRequest(format!("unknown font_id `{}`", label.font_id)))?;
        let font = sl_map_apis::text::load_font(font_path)?;
        let color = parse_color(label.color.trim())?;
        let scale = ab_glyph::PxScale::from(label.font_px);
        let (text_w, text_h) = sl_map_apis::text::measure_text(scale, &font, &lines);
        if text_w > rect.width || text_h > rect.height {
            return Err(Error::BadRequest(format!(
                "label text renders at {text_w}x{text_h} px but the free area at slot `{anchor}` only has {}x{} px",
                rect.width, rect.height
            )));
        }
        // Align within the free rectangle: the label's own value, else the
        // slot's outward default.
        let (default_h, default_v) = anchor.default_alignment();
        let h = parse_h_align(label.h_align.as_deref())?.unwrap_or(default_h);
        let v = parse_v_align(label.v_align.as_deref())?.unwrap_or(default_v);
        let origin_x = rect.x.saturating_add(h.offset(text_w, rect.width));
        let origin_y = rect.y.saturating_add(v.offset(text_h, rect.height));
        draws.push(LabelDraw {
            lines,
            font,
            style: sl_map_apis::text::LabelStyle {
                scale,
                fg: color,
                shadow: Rgba([0, 0, 0, 180]),
                // Each line is aligned within the block by the label's own
                // horizontal alignment (the block is then placed in the slot).
                align: h,
            },
            origin: (
                i32::try_from(origin_x).unwrap_or(0),
                i32::try_from(origin_y).unwrap_or(0),
            ),
        });
    }
    Ok((draws, used))
}

/// Draw a planned label list onto `map` (above the route and GLW overlay).
fn execute_labels(draws: &[LabelDraw], map: &mut Map) {
    use sl_map_apis::map_tiles::MapLike as _;
    for d in draws {
        map.draw_text_label(d.origin, &d.lines, &d.style, &d.font);
    }
}

/// Plan and draw labels onto `map` in one step, measuring free space on `map`
/// itself. Used by tests where the draw target is also a faithful occupancy
/// source (a transparent overlay map); the real render measures occupancy on a
/// separate overlay-only map via [`plan_labels`] + [`execute_labels`] because
/// its base tiles are opaque.
#[cfg(test)]
fn draw_labels_on_map(
    fonts: &crate::fonts::FontDirectory,
    labels: &[TextLabel],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    reserved_by_others: &[sl_map_apis::coverage::PlacementSlot],
    map: &mut Map,
) -> Result<Vec<sl_map_apis::coverage::PlacementSlot>, Error> {
    let (draws, used) = plan_labels(fonts, labels, legend_slot, reserved_by_others, map)?;
    execute_labels(&draws, map);
    Ok(used)
}

/// Composite the logo images onto `map`, last (above the route, GLW overlay
/// and labels). Each logo is drawn at its native pixel size (optionally
/// integer-doubled with nearest-neighbour sampling), alpha-blended, and
/// aligned within its (single-slot or spanned) free rectangle. Rejects (as a
/// `BadRequest`) any logo that overflows its free space, has an invalid
/// scale, or whose reserved slots clash with the legend, another logo, or a
/// slot already reserved by a label (`reserved_by_others`). Returns the set
/// of slots the logos reserved.
/// Plan the logo placements against the free space measured on `occupancy`
/// (an overlay-only map). Each logo is loaded, decoded, optionally
/// integer-doubled (nearest-neighbour), and aligned within its (single-slot or
/// spanned) free rectangle. Rejects (as a `BadRequest`) any logo that overflows
/// its free space, has an invalid scale, or whose reserved slots clash with the
/// legend, another logo, or a slot already reserved by a label
/// (`reserved_by_others`). Returns the draw list (for [`execute_logos`]) and
/// the set of slots the logos reserved.
async fn plan_logos(
    state: &AppState,
    logos: &[LogoPlacement],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    reserved_by_others: &[sl_map_apis::coverage::PlacementSlot],
    occupancy: &Map,
) -> Result<(Vec<LogoDraw>, Vec<sl_map_apis::coverage::PlacementSlot>), Error> {
    let mut used: Vec<sl_map_apis::coverage::PlacementSlot> = Vec::new();
    let mut draws: Vec<LogoDraw> = Vec::new();
    if logos.is_empty() {
        return Ok((draws, used));
    }
    let grid = sl_map_apis::coverage::OccupancyGrid::from_map(
        occupancy,
        sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
    );
    let slots = grid.evaluate_slots();
    // Validate + reserve + load + decode + scale + fit, so nothing is drawn
    // until every logo has been accepted.
    for logo in logos {
        if logo.scale != 1 && logo.scale != 2 && logo.scale != 4 {
            return Err(Error::BadRequest(format!(
                "logo scale must be 1, 2 or 4, got {}",
                logo.scale
            )));
        }
        let anchor: sl_map_apis::coverage::PlacementSlot =
            logo.slot
                .parse()
                .map_err(|err: sl_map_apis::coverage::ParsePlacementSlotError| {
                    Error::BadRequest(err.to_string())
                })?;
        let group = parse_slot_group(anchor, &logo.slots)?;
        let (reserved, rect) = resolve_placement(anchor, &group, &slots, &grid)?;
        reserve(
            &reserved,
            &mut used,
            reserved_by_others,
            legend_slot,
            "logo",
        )?;
        // Look up the stored file (read permission was checked at submit
        // time; the saved_render_logos link prevents deletion meanwhile).
        let row: Option<(String,)> =
            sqlx::query_as("SELECT image_filename FROM saved_logos WHERE logo_id = ?1")
                .bind(logo.logo_id.as_bytes().to_vec())
                .fetch_optional(&state.db)
                .await
                .map_err(|err| {
                    tracing::error!("logo file lookup failed: {err}");
                    Error::Database
                })?;
        let (image_filename,) = row
            .ok_or_else(|| Error::BadRequest(format!("logo {} no longer exists", logo.logo_id)))?;
        let bytes = storage::read_logo_file(&state.config.storage_dir, &image_filename).await?;
        let decoded = image::load_from_memory(&bytes).map_err(|e| {
            Error::BadRequest(format!("could not decode logo {}: {e}", logo.logo_id))
        })?;
        let mut rgba = decoded.to_rgba8();
        if logo.scale != 1 {
            let factor = u32::from(logo.scale);
            let (w, h) = (rgba.width(), rgba.height());
            rgba = image::imageops::resize(
                &rgba,
                w.saturating_mul(factor),
                h.saturating_mul(factor),
                image::imageops::FilterType::Nearest,
            );
        }
        let (w, h) = (rgba.width(), rgba.height());
        if w > rect.width || h > rect.height {
            return Err(Error::BadRequest(format!(
                "logo renders at {w}x{h} px but the free area at slot `{anchor}` only has {}x{} px",
                rect.width, rect.height
            )));
        }
        let (default_h, default_v) = anchor.default_alignment();
        let hh = parse_h_align(logo.h_align.as_deref())?.unwrap_or(default_h);
        let vv = parse_v_align(logo.v_align.as_deref())?.unwrap_or(default_v);
        let origin_x = rect.x.saturating_add(hh.offset(w, rect.width));
        let origin_y = rect.y.saturating_add(vv.offset(h, rect.height));
        draws.push(LogoDraw {
            img: rgba,
            x: i64::from(origin_x),
            y: i64::from(origin_y),
        });
    }
    Ok((draws, used))
}

/// Composite a planned logo list onto `map` with alpha blending (above the
/// route, GLW overlay and labels).
fn execute_logos(draws: &[LogoDraw], map: &mut Map) {
    use sl_map_apis::map_tiles::MapLike as _;
    for d in draws {
        image::imageops::overlay(map.image_mut(), &d.img, d.x, d.y);
    }
}

/// Resolve a notecard to its route's grid rectangle: the bare bounding box of
/// the waypoints plus the configured border padding. Shared by the submit-time
/// placement check and the render job so both measure the same rectangle.
async fn resolve_notecard_rect(
    state: &AppState,
    notecard: &USBNotecard,
    borders: (u16, u16, u16, u16),
) -> Result<(GridRectangle, GridRectangle), Error> {
    let bare_rect = {
        let mut region = state.region_cache.lock().await;
        usb_notecard_to_grid_rectangle(&mut region, notecard).await?
    };
    let (border_north, border_south, border_east, border_west) = borders;
    let rect = bare_rect
        .expanded_west(border_west)
        .expanded_east(border_east)
        .expanded_south(border_south)
        .expanded_north(border_north);
    Ok((bare_rect, rect))
}

/// Build the overlay-only occupancy map (a blank base sized to the final image,
/// plus the GLW shapes and — for notecard renders — the route) and plan the
/// labels and logos against it, returning the draws to paint.
///
/// This is the single source of truth for placement fit. The submit handlers
/// call it before persisting a render so an over-full slot is rejected up front
/// (nothing is saved), and the render jobs call it to produce the draws they
/// composite onto the real map. Sharing one routine guarantees the pre-save
/// check and the job can never disagree about whether a placement fits. The
/// occupancy is measured on a blank base (the opaque map tiles would otherwise
/// count as fully covered), so this needs no map-tile fetch.
async fn plan_placements(
    state: &AppState,
    occ_rect: GridRectangle,
    common: &CommonParams,
    glw_ctx: Option<&GlwJobCtx>,
    route: Option<(&USBNotecard, Rgba<u8>)>,
    labels: &[TextLabel],
    logos: &[LogoPlacement],
) -> Result<(Vec<LabelDraw>, Vec<LogoDraw>), Error> {
    let legend_slot = legend_slot_of(glw_ctx);
    let occ = {
        let mut occ = Map::blank_fit(occ_rect, common.max_width, common.max_height)?;
        if let Some(ctx) = glw_ctx {
            apply_glw_overlay_readonly(state, ctx.created_by, &ctx.options, &mut occ).await?;
        }
        if let Some((notecard, color)) = route {
            let mut region = state.region_cache.lock().await;
            occ.draw_route_with_progress(&mut region, notecard, color, None)
                .await?;
        }
        occ
    };
    let (label_draws, label_slots) = plan_labels(&state.fonts, labels, legend_slot, &[], &occ)?;
    let (logo_draws, _) = plan_logos(state, logos, legend_slot, &label_slots, &occ).await?;
    Ok((label_draws, logo_draws))
}

/// Validate that every logo placement references a logo the user may read
/// and that lives in the same library scope as the render (matching the
/// notecard/GLW same-scope invariant). Returns the de-duplicated list of
/// logo ids to link onto the render row.
async fn validate_logos(
    state: &AppState,
    current_user: Uuid,
    destination: Destination,
    logos: &[LogoPlacement],
) -> Result<Vec<Uuid>, Error> {
    let mut ids: Vec<Uuid> = Vec::new();
    for logo in logos {
        let row = library::assert_can_read_logo(&state.db, current_user, logo.logo_id).await?;
        let logo_dest = library::destination_from_columns(
            row.owner_user_id.clone(),
            row.owner_group_id.clone(),
        )?;
        if logo_dest != destination {
            return Err(Error::BadRequest(format!(
                "logo {} is not in the same library as this render; copy it into the render's \
                 scope first",
                logo.logo_id
            )));
        }
        if !ids.contains(&logo.logo_id) {
            ids.push(logo.logo_id);
        }
    }
    Ok(ids)
}

/// Authorize *read* access to every logo referenced by a placement set,
/// without the same-library requirement [`validate_logos`] additionally
/// enforces. Used by the read-only placement-preview endpoints: they carry no
/// render destination (so the same-scope check does not apply), but must still
/// refuse to composite — and thereby disclose the pixels of — a logo the
/// current user cannot see. [`plan_logos`] itself loads the file by id with no
/// authorization, trusting its callers to have gated read access first.
async fn assert_can_read_logos(
    state: &AppState,
    current_user: Uuid,
    logos: &[LogoPlacement],
) -> Result<(), Error> {
    for logo in logos {
        library::assert_can_read_logo(&state.db, current_user, logo.logo_id).await?;
    }
    Ok(())
}

/// Link the render's logos, marking the freshly-inserted render row `failed`
/// if the link step errors. Without this a [`link_render_logos`] failure
/// between [`insert_render_row`] and spawning the job would strand the
/// `in_progress` row forever — no job is ever spawned to fail it, so it would
/// show as a perpetual spinner and keep counting against the user's
/// concurrent-render cap.
async fn link_render_logos_or_fail(
    state: &AppState,
    render_id: Uuid,
    logo_ids: &[Uuid],
) -> Result<(), Error> {
    if let Err(err) = link_render_logos(state, render_id, logo_ids).await {
        update_failed(
            state,
            render_id,
            &format!("linking render logos failed: {err}"),
            Utc::now(),
        )
        .await;
        return Err(err);
    }
    Ok(())
}

/// Insert the `saved_render_logos` link rows for a render. Idempotent per
/// (render, logo) pair.
async fn link_render_logos(
    state: &AppState,
    render_id: Uuid,
    logo_ids: &[Uuid],
) -> Result<(), Error> {
    for id in logo_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO saved_render_logos (render_id, logo_id) VALUES (?1, ?2)",
        )
        .bind(render_id.as_bytes().to_vec())
        .bind(id.as_bytes().to_vec())
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("link render logo failed: {err}");
            Error::Database
        })?;
    }
    Ok(())
}

/// Rewrite a `saved_renders.settings_json` so the carried GLW source
/// becomes a stable `SavedId` pointing at the resolved row. Returns
/// `Ok(None)` if the existing settings either has no GLW field or
/// already carries a `SavedId` (then the original JSON is left alone).
async fn rewrite_settings_json_with_saved_glw(
    state: &AppState,
    render_id: Uuid,
    glw_data_id: Uuid,
) -> Result<Option<String>, Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT settings_json FROM saved_renders WHERE render_id = ?1")
            .bind(render_id.as_bytes().to_vec())
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                tracing::error!("settings_json fetch failed: {err}");
                Error::Database
            })?;
    let Some((settings_json,)) = row else {
        return Ok(None);
    };
    let mut parsed: SavedRenderSettings = serde_json::from_str(&settings_json)?;
    let glw_slot: &mut Option<GlwRenderOptions> = match &mut parsed {
        SavedRenderSettings::GridRectangle(s) => &mut s.glw,
        SavedRenderSettings::UsbNotecard(s) => &mut s.glw,
    };
    let Some(opts) = glw_slot.as_mut() else {
        return Ok(None);
    };
    // No-op if it's already SavedId (e.g. the user submitted a SavedId
    // and the worker simply re-read the row).
    if matches!(opts.source, GlwSource::SavedId { .. }) {
        return Ok(None);
    }
    opts.source = GlwSource::SavedId { glw_data_id };
    let json = serde_json::to_string(&parsed)?;
    Ok(Some(json))
}

#[cfg(test)]
mod placement_slots_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn blank_map() -> Result<Map, Box<dyn std::error::Error>> {
        // a 4x4 region rectangle at zoom 4 -> 128x128 pixels
        let rect = GridRectangle::new(
            GridCoordinates::new(1000, 1000),
            GridCoordinates::new(1003, 1003),
        );
        Ok(Map::blank(rect, ZoomLevel::try_new(4)?))
    }

    #[test]
    fn empty_map_reports_nine_free_slots() -> Result<(), Box<dyn std::error::Error>> {
        let map = blank_map()?;
        let resp = compute_placement_slots(&map, &[]);
        assert_eq!(resp.slots.len(), 9);
        assert_eq!((resp.image_width, resp.image_height), (128, 128));
        // with no overlay drawn (and the legend excluded by design) every
        // anchor is free, including the legend's default top-left corner
        for s in &resp.slots {
            assert!(s.available, "{} should be free", s.slot);
        }
        let top_left = resp
            .slots
            .iter()
            .find(|s| s.slot == "top_left")
            .ok_or("missing top_left slot")?;
        assert!(top_left.free_rect.is_some());
        Ok(())
    }

    #[test]
    fn route_blocks_centre_slot() -> Result<(), Box<dyn std::error::Error>> {
        let mut map = blank_map()?;
        map.draw_pixel_waypoint_route(
            &[(10f32, 10f32), (64f32, 64f32), (118f32, 118f32)],
            Rgba([255, 0, 0, 255]),
        )?;
        let resp = compute_placement_slots(&map, &[]);
        let center = resp
            .slots
            .iter()
            .find(|s| s.slot == "center")
            .ok_or("missing center slot")?;
        assert!(!center.available);
        let top_right = resp
            .slots
            .iter()
            .find(|s| s.slot == "top_right")
            .ok_or("missing top_right slot")?;
        assert!(top_right.available);
        Ok(())
    }

    #[test]
    fn requested_group_reports_combined_rect() -> Result<(), Box<dyn std::error::Error>> {
        use sl_map_apis::coverage::PlacementSlot as P;
        let map = blank_map()?;
        let resp = compute_placement_slots(&map, &[vec![P::TopLeft, P::TopCenter]]);
        assert_eq!(resp.groups.len(), 1);
        let g = resp.groups.first().ok_or("missing group")?;
        assert_eq!(g.slots, vec!["top_left", "top_center"]);
        assert!(g.available && g.free_rect.is_some());
        // the combined rect is wider than either single top slot
        let tl = resp
            .slots
            .iter()
            .find(|s| s.slot == "top_left")
            .ok_or("missing top_left")?;
        assert!(g.free_width > tl.free_width);
        Ok(())
    }
}

#[cfg(test)]
mod glw_preview_tests {
    //! Tests for the drawing the `glw_preview` handler performs once the event
    //! is resolved: rendering the GLW overlay onto a transparent blank map at
    //! the preview zoom, with the legend excluded (it is placed separately by
    //! the placement-slot logic). The resolution / HTTP plumbing needs a full
    //! `AppState`, so these cover the pure drawing core instead.
    use super::*;
    use pretty_assertions::assert_eq;
    use sl_glw::{GlwEvent, MapLikeGlwExt as _};

    /// Sample event with one area and one circle (same shape as the
    /// `sl-glw` render smoke test's fixture).
    const SAMPLE_JSON: &str = r#"{
        "eventId": 6910,
        "eventName": "test cruise",
        "eventKey": "key cruise",
        "directorName": "LaliaCasau Resident",
        "directorKey": "b609826a-b167-41e0-8e67-9fc0e78b97a1",
        "base": {
            "wind": { "dir": 175, "speed": 17, "gusts": 8, "shifts": 5, "period": 90 },
            "waves": {
                "height": 1.5, "speed": 3, "length": 35,
                "heightVar": 5, "lengthVar": 5,
                "effects": { "speed": 1, "steer": 1 }
            },
            "currents": { "speed": 0, "dir": 180, "waterDepth": 0 }
        },
        "areas": {
            "area1": {
                "coordSW": { "x": 1133, "y": 1048 },
                "coordNE": { "x": 1135, "y": 1049 },
                "margin": 25, "overlap": 0,
                "currents": { "speed": 1, "dir": 225, "waterDepth": 8 }
            }
        },
        "circles": {
            "circle1": {
                "centerSim": { "x": 1136, "y": 1051 },
                "centerPoint": { "x": 90, "y": 175 },
                "radius": 127, "margin": 25, "overlap": 0,
                "wind": { "speed": 15 },
                "currents": { "speed": 0.1, "dir": 225, "waterDepth": 6 }
            }
        }
    }"#;

    /// A `FontDirectory` scanning the workspace root for the bundled
    /// `DejaVuSans.ttf`.
    fn test_font() -> Result<ab_glyph::FontVec, Box<dyn std::error::Error>> {
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
        let fonts = crate::fonts::FontDirectory::scan(root)?;
        let path = fonts
            .path_for("DejaVuSans.ttf")
            .ok_or("bundled DejaVuSans.ttf font is missing")?;
        Ok(sl_map_apis::text::load_font(path)?)
    }

    /// The blank map the preview overlay is drawn onto: a transparent RGBA
    /// image whose dimensions are `pixels_per_region(zoom) * size`, matching
    /// the `boundsW`/`boundsH` the client computes for the bounds rectangle.
    /// This is the alignment contract between the overlay PNG and the tiles.
    #[test]
    fn blank_overlay_dimensions_match_client_bounds() -> Result<(), Box<dyn std::error::Error>> {
        let rect = GridRectangle::new(
            GridCoordinates::new(1130, 1045),
            GridCoordinates::new(1140, 1055),
        );
        let zoom = ZoomLevel::try_new(4)?;
        let map = Map::blank(rect, zoom);
        let (w, h) = image::GenericImageView::dimensions(&map);
        // 11 sims each way × 32 px/region at zoom 4.
        assert_eq!((w, h), (11 * 32, 11 * 32));
        // a fresh blank overlay is fully transparent, so it composites over the
        // tiles without altering them until shapes are drawn
        for y in 0..h {
            for x in 0..w {
                let image::Rgba([_, _, _, a]) = image::GenericImageView::get_pixel(&map, x, y);
                assert_eq!(a, 0, "blank overlay must start fully transparent");
            }
        }
        Ok(())
    }

    /// Whether any non-transparent pixel falls inside the top-left `n×n`
    /// corner, where the default-slot legend would be drawn and where the
    /// sample event has no shapes (all shapes sit at sim x ≥ 1133, i.e. ≥ 96 px
    /// from the left).
    fn top_left_has_pixels(map: &Map, n: u32) -> bool {
        for y in 0..n {
            for x in 0..n {
                let image::Rgba([_, _, _, a]) = image::GenericImageView::get_pixel(map, x, y);
                if a != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// The preview draws the shapes but omits the legend: drawing the event
    /// with the legend enabled fills the top-left corner, while the preview's
    /// legend-disabled style leaves that corner untouched.
    #[test]
    fn preview_overlay_omits_the_legend() -> Result<(), Box<dyn std::error::Error>> {
        let event: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;
        let font = test_font()?;
        let rect = GridRectangle::new(
            GridCoordinates::new(1130, 1045),
            GridCoordinates::new(1140, 1055),
        );
        let zoom = ZoomLevel::try_new(4)?;

        // Baseline: the legend in its default top-left slot writes pixels into
        // the top-left corner.
        let mut with_legend = Map::blank(rect.clone(), zoom);
        let style_with = build_glw_style(&GlwStyleOverrides::default(), Some("top_left"))?;
        with_legend.draw_glw_event_with_font(&event, &style_with, &font)?;
        assert!(
            top_left_has_pixels(&with_legend, 40),
            "the legend should fill the top-left corner when enabled"
        );

        // Preview style: legend disabled exactly as `glw_preview` does. The
        // shape-free top-left corner stays fully transparent.
        let mut without_legend = Map::blank(rect, zoom);
        let mut style_without = build_glw_style(&GlwStyleOverrides::default(), None)?;
        style_without.legend_position = None;
        without_legend.draw_glw_event_with_font(&event, &style_without, &font)?;
        assert!(
            !top_left_has_pixels(&without_legend, 40),
            "the preview overlay must not draw the legend"
        );

        // The shapes themselves are still drawn (the overlay isn't empty).
        let (w, h) = image::GenericImageView::dimensions(&without_legend);
        let mut any = false;
        'outer: for y in 0..h {
            for x in 0..w {
                let image::Rgba([_, _, _, a]) =
                    image::GenericImageView::get_pixel(&without_legend, x, y);
                if a != 0 {
                    any = true;
                    break 'outer;
                }
            }
        }
        assert!(any, "the GLW shapes should still be drawn");
        Ok(())
    }
}

#[cfg(test)]
mod route_preview_tests {
    //! Tests for the drawing the `route_preview` handler performs: converting
    //! already-resolved waypoints to pixel coordinates and drawing the route
    //! (spline + arrows, in the route colour) onto a blank final-image-sized
    //! map — the very same `pixel_coordinates_for_coordinates` +
    //! `draw_pixel_waypoint_route` calls the handler makes. The auth / HTTP
    //! plumbing needs a full `AppState`, so these cover the pure drawing core.
    use super::*;
    use pretty_assertions::assert_eq;
    use sl_map_apis::map_tiles::MapLike as _;

    /// Distinctive opaque colour the route is drawn in. The spline rectangles
    /// and arrow polygons write it verbatim (no anti-aliasing), so it can be
    /// matched exactly.
    const ROUTE_COLOR: Rgba<u8> = Rgba([255, 0, 0, 255]);

    /// Pixel coordinates are bounded by the (capped) output dimensions, so they
    /// never approach `f32`'s 2^23 exact-integer ceiling — the same widening
    /// `draw_route_with_progress` and the handler perform.
    #[expect(
        clippy::as_conversions,
        clippy::cast_precision_loss,
        reason = "pixel coordinates are bounded by the output dimensions and never approach 2^23"
    )]
    fn widen(v: u32) -> f32 {
        v as f32
    }

    /// Convert one resolved waypoint `(region_x, region_y, x, y)` to pixel
    /// coordinates exactly as the `route_preview` handler does.
    fn pixel_for(map: &Map, rx: u16, ry: u16, x: f32, y: f32) -> Option<(f32, f32)> {
        let (px, py) = map.pixel_coordinates_for_coordinates(
            &GridCoordinates::new(rx, ry),
            &RegionCoordinates::new(x, y, 0f32),
        )?;
        Some((widen(px), widen(py)))
    }

    /// Shortest distance from point `p` to the line segment `a`–`b`.
    fn distance_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
        let (px, py) = p;
        let (ax, ay) = a;
        let (bx, by) = b;
        let (dx, dy) = (bx - ax, by - ay);
        let len_sq = (dx * dx) + (dy * dy);
        let t = if len_sq <= f32::EPSILON {
            0f32
        } else {
            ((((px - ax) * dx) + ((py - ay) * dy)) / len_sq).clamp(0f32, 1f32)
        };
        let (cx, cy) = (ax + (t * dx), ay + (t * dy));
        ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
    }

    /// The route is drawn in the requested colour over an otherwise fully
    /// transparent background: this is the alignment + colour contract the
    /// client relies on when compositing the PNG over the tiles.
    #[test]
    fn route_drawn_in_colour_on_transparent_background() -> Result<(), Box<dyn std::error::Error>> {
        let rect = GridRectangle::new(
            GridCoordinates::new(1000, 1000),
            GridCoordinates::new(1010, 1010),
        );
        let mut map = Map::blank_fit(rect, 512, 512)?;
        let pixel_waypoints = vec![
            pixel_for(&map, 1001, 1001, 128f32, 128f32).ok_or("waypoint outside rect")?,
            pixel_for(&map, 1009, 1009, 128f32, 128f32).ok_or("waypoint outside rect")?,
        ];
        map.draw_pixel_waypoint_route(&pixel_waypoints, ROUTE_COLOR)?;

        let (w, h) = image::GenericImageView::dimensions(&map);
        let mut coloured = 0u32;
        let mut foreign = 0u32;
        for y in 0..h {
            for x in 0..w {
                let image::Rgba([r, g, b, a]) = image::GenericImageView::get_pixel(&map, x, y);
                if image::Rgba([r, g, b, a]) == ROUTE_COLOR {
                    coloured += 1;
                } else if a != 0 {
                    // any non-transparent pixel that isn't the route colour
                    foreign += 1;
                }
            }
        }
        assert!(
            coloured > 0,
            "the route must be drawn in the requested colour"
        );
        assert_eq!(
            foreign, 0,
            "only the route colour and full transparency may appear"
        );
        Ok(())
    }

    /// Three waypoints forming a peak are joined by a Catmull-Rom spline, not
    /// straight segments: at least one route pixel lies clearly off both
    /// straight chords between consecutive waypoints. This is what makes the
    /// preview match the curved output instead of the old straight polyline.
    #[test]
    fn route_curves_between_waypoints() -> Result<(), Box<dyn std::error::Error>> {
        let rect = GridRectangle::new(
            GridCoordinates::new(1000, 1000),
            GridCoordinates::new(1012, 1012),
        );
        let mut map = Map::blank_fit(rect, 512, 512)?;
        // A ∧-shaped arc: ends low, middle high. The tangent at the middle is
        // horizontal, so the spline bows away from the straight chords.
        let p0 = pixel_for(&map, 1001, 1001, 128f32, 128f32).ok_or("waypoint outside rect")?;
        let p1 = pixel_for(&map, 1006, 1006, 128f32, 128f32).ok_or("waypoint outside rect")?;
        let p2 = pixel_for(&map, 1011, 1001, 128f32, 128f32).ok_or("waypoint outside rect")?;
        map.draw_pixel_waypoint_route(&[p0, p1, p2], ROUTE_COLOR)?;

        let (w, h) = image::GenericImageView::dimensions(&map);
        let mut max_off = 0f32;
        for y in 0..h {
            for x in 0..w {
                if image::GenericImageView::get_pixel(&map, x, y) == ROUTE_COLOR {
                    let p = (widen(x), widen(y));
                    let off = distance_to_segment(p, p0, p1).min(distance_to_segment(p, p1, p2));
                    max_off = max_off.max(off);
                }
            }
        }
        assert!(
            max_off > 4f32,
            "the route should curve away from the straight chords (max off-chord \
             distance was {max_off:.1} px); a straight polyline would stay on them"
        );
        Ok(())
    }
}

#[cfg(test)]
mod glw_legend_preview_tests {
    //! Tests for the drawing `glw_legend_preview` performs: rendering ONLY the
    //! base legend onto a blank final-image-sized map at its chosen slot. The
    //! resolution / HTTP plumbing needs a full `AppState`, so these cover the
    //! pure drawing core (the same `draw_glw_base_legend` call the handler
    //! makes) instead.
    use super::*;
    use pretty_assertions::assert_eq;
    use sl_glw::{GlwEvent, MapLikeGlwExt as _};

    /// Minimal event with a non-zero base block (the legend only reads
    /// `base`); the empty `areas`/`circles` keep the legend the only content.
    const SAMPLE_JSON: &str = r#"{
        "eventId": 6910,
        "eventName": "test cruise",
        "eventKey": "key cruise",
        "directorName": "LaliaCasau Resident",
        "directorKey": "b609826a-b167-41e0-8e67-9fc0e78b97a1",
        "base": {
            "wind": { "dir": 175, "speed": 17, "gusts": 8, "shifts": 5, "period": 90 },
            "waves": {
                "height": 1.5, "speed": 3, "length": 35,
                "heightVar": 5, "lengthVar": 5,
                "effects": { "speed": 1, "steer": 1 }
            },
            "currents": { "speed": 0, "dir": 180, "waterDepth": 0 }
        },
        "areas": {},
        "circles": {}
    }"#;

    /// A font loaded from the workspace's bundled `DejaVuSans.ttf`.
    fn test_font() -> Result<ab_glyph::FontVec, Box<dyn std::error::Error>> {
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
        let fonts = crate::fonts::FontDirectory::scan(root)?;
        let path = fonts
            .path_for("DejaVuSans.ttf")
            .ok_or("bundled DejaVuSans.ttf font is missing")?;
        Ok(sl_map_apis::text::load_font(path)?)
    }

    /// True if any non-transparent pixel falls inside the `w`×`h` corner at
    /// `(ox, oy)`.
    fn corner_has_pixels(map: &Map, ox: u32, oy: u32, w: u32, h: u32) -> bool {
        for y in oy..oy.saturating_add(h) {
            for x in ox..ox.saturating_add(w) {
                let image::Rgba([_, _, _, a]) = image::GenericImageView::get_pixel(map, x, y);
                if a != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// The legend is drawn only in its chosen slot and nowhere else: a
    /// `bottom_right` legend fills the bottom-right corner while leaving the
    /// top-left corner transparent, and no shapes are drawn (legend-only).
    #[test]
    fn legend_drawn_only_in_chosen_slot() -> Result<(), Box<dyn std::error::Error>> {
        let event: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;
        let font = test_font()?;
        let rect = GridRectangle::new(
            GridCoordinates::new(1130, 1045),
            GridCoordinates::new(1140, 1055),
        );
        let mut map = Map::blank_fit(rect, 512, 512)?;
        let style = build_glw_style(&GlwStyleOverrides::default(), Some("bottom_right"))?;
        // Exactly what the handler does: draw the legend alone (no shapes).
        map.draw_glw_base_legend(&event.base, &style, &font)?;
        let (w, h) = image::GenericImageView::dimensions(&map);
        assert!(
            corner_has_pixels(&map, w.saturating_sub(80), h.saturating_sub(80), 80, 80),
            "the bottom_right legend should fill the bottom-right corner"
        );
        assert!(
            !corner_has_pixels(&map, 0, 0, 80, 80),
            "no legend should appear in the unselected top-left corner"
        );
        Ok(())
    }

    /// A `none` legend slot yields `legend_position == None`, so the handler
    /// skips drawing and the map stays fully transparent.
    #[test]
    fn legend_slot_none_draws_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let event: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;
        let font = test_font()?;
        let rect = GridRectangle::new(
            GridCoordinates::new(1130, 1045),
            GridCoordinates::new(1140, 1055),
        );
        let mut map = Map::blank_fit(rect, 512, 512)?;
        let style = build_glw_style(&GlwStyleOverrides::default(), Some("none"))?;
        assert!(
            style.legend_position.is_none(),
            "the `none` slot must disable the legend"
        );
        // The handler only calls draw when legend_position.is_some(); mirror
        // that guard, then assert nothing was drawn.
        if style.legend_position.is_some() {
            map.draw_glw_base_legend(&event.base, &style, &font)?;
        }
        let (w, h) = image::GenericImageView::dimensions(&map);
        for y in 0..h {
            for x in 0..w {
                let image::Rgba([_, _, _, a]) = image::GenericImageView::get_pixel(&map, x, y);
                assert_eq!(a, 0, "a `none` legend slot must leave the map transparent");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use pretty_assertions::assert_matches;
    use sl_map_apis::coverage::{HAlign, PlacementSlot, VAlign};

    /// A `FontDirectory` scanning the workspace root, which contains the
    /// bundled `DejaVuSans.ttf` (font id `DejaVuSans.ttf`).
    fn test_fonts() -> Result<crate::fonts::FontDirectory, Box<dyn std::error::Error>> {
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
        Ok(crate::fonts::FontDirectory::scan(root)?)
    }

    /// A blank 352x352 RGBA map (11 sims at zoom 4) — every slot fully free.
    fn blank_map() -> Result<Map, Box<dyn std::error::Error>> {
        let rect = GridRectangle::new(
            GridCoordinates::new(1130, 1130),
            GridCoordinates::new(1140, 1140),
        );
        Ok(Map::blank(rect, ZoomLevel::try_new(4)?))
    }

    fn label(slot: &str, font_px: f32, lines: &[&str]) -> TextLabel {
        TextLabel {
            slot: slot.to_owned(),
            lines: lines.iter().map(|s| (*s).to_owned()).collect(),
            font_id: "DejaVuSans.ttf".to_owned(),
            font_px,
            color: "#ffffff".to_owned(),
            h_align: None,
            v_align: None,
            slots: Vec::new(),
        }
    }

    fn any_pixel_drawn(map: &Map) -> bool {
        let (w, h) = image::GenericImageView::dimensions(map);
        for y in 0..h {
            for x in 0..w {
                let image::Rgba([_, _, _, a]) = image::GenericImageView::get_pixel(map, x, y);
                if a != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// A copy of `blank_map` filled fully opaque, modelling the real render's
    /// base tiles (`new_with_progress` builds an opaque RGB image).
    fn opaque_map() -> Result<Map, Box<dyn std::error::Error>> {
        use image::GenericImage as _;
        use sl_map_apis::map_tiles::MapLike as _;
        let mut map = blank_map()?;
        let (w, h) = image::GenericImageView::dimensions(&map);
        for y in 0..h {
            for x in 0..w {
                map.image_mut().put_pixel(x, y, image::Rgba([5, 5, 5, 255]));
            }
        }
        Ok(map)
    }

    /// The split between planning (occupancy) and drawing (target) is what lets
    /// the real render place labels: planning against an overlay-only map finds
    /// free space and the result draws onto an opaque target, whereas planning
    /// against the opaque map itself finds no room (the bug the split fixes).
    #[test]
    fn labels_plan_on_overlay_then_draw_onto_opaque_target()
    -> Result<(), Box<dyn std::error::Error>> {
        let fonts = test_fonts()?;
        let lbls = [label("top_left", 18f32, &["Hi"])];

        // Overlay-only occupancy (a transparent map) → the slot is free, the
        // label is planned, and it draws onto an opaque target.
        let occ = blank_map()?;
        let (draws, used) = plan_labels(&fonts, &lbls, None, &[], &occ)?;
        assert!(!draws.is_empty(), "the label should be planned");
        assert!(used.contains(&PlacementSlot::TopLeft));
        let mut target = opaque_map()?;
        execute_labels(&draws, &mut target);
        let (w, h) = image::GenericImageView::dimensions(&target);
        let mut changed = false;
        'outer: for y in 0..h {
            for x in 0..w {
                let image::Rgba([r, g, b, _]) = image::GenericImageView::get_pixel(&target, x, y);
                if (r, g, b) != (5, 5, 5) {
                    changed = true;
                    break 'outer;
                }
            }
        }
        assert!(changed, "the label must draw onto the opaque target");

        // Planning against the opaque map directly finds no free space.
        let opaque_occ = opaque_map()?;
        assert!(
            plan_labels(&fonts, &lbls, None, &[], &opaque_occ).is_err(),
            "an opaque occupancy map leaves no room — the regression the split fixes"
        );
        Ok(())
    }

    #[test]
    fn legend_position_from_slot_maps_names() -> Result<(), Box<dyn std::error::Error>> {
        use sl_map_apis::coverage::PlacementSlot as P;
        assert_eq!(legend_position_from_slot(None)?, Some(P::TopLeft));
        assert_eq!(legend_position_from_slot(Some(""))?, Some(P::TopLeft));
        assert_eq!(legend_position_from_slot(Some("none"))?, None);
        assert_eq!(legend_position_from_slot(Some("center"))?, Some(P::Center));
        assert_eq!(
            legend_position_from_slot(Some("bottom_right"))?,
            Some(P::BottomRight)
        );
        assert_matches!(legend_position_from_slot(Some("nonsense")), Err(_));
        Ok(())
    }

    #[test]
    fn slot_parsers_and_alignment() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            "middle_left".parse::<PlacementSlot>()?,
            PlacementSlot::MiddleLeft
        );
        assert_matches!("nope".parse::<PlacementSlot>(), Err(_));
        assert_eq!(parse_h_align(None)?, None);
        assert_eq!(parse_h_align(Some("right"))?, Some(HAlign::Right));
        assert_eq!(parse_v_align(Some("bottom"))?, Some(VAlign::Bottom));
        assert_matches!(parse_h_align(Some("sideways")), Err(_));
        Ok(())
    }

    #[test]
    fn small_label_draws_and_oversized_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let fonts = test_fonts()?;

        let mut map = blank_map()?;
        draw_labels_on_map(
            &fonts,
            &[label("top_left", 18f32, &["Hi"])],
            None,
            &[],
            &mut map,
        )?;
        assert!(any_pixel_drawn(&map), "a fitting label should draw pixels");

        // a font far too large to fit the 352px map is rejected
        let mut map2 = blank_map()?;
        let err = draw_labels_on_map(
            &fonts,
            &[label("center", 4000f32, &["BIG"])],
            None,
            &[],
            &mut map2,
        );
        assert_matches!(
            err,
            Err(Error::BadRequest(_)),
            "oversized label must be rejected"
        );
        Ok(())
    }

    #[test]
    fn slot_collisions_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let fonts = test_fonts()?;

        // two labels in the same slot
        let mut map = blank_map()?;
        let dup = draw_labels_on_map(
            &fonts,
            &[
                label("top_right", 16f32, &["a"]),
                label("top_right", 16f32, &["b"]),
            ],
            None,
            &[],
            &mut map,
        );
        assert_matches!(
            dup,
            Err(Error::BadRequest(_)),
            "duplicate slot must be rejected"
        );

        // a label sharing the legend's slot
        let mut map2 = blank_map()?;
        let clash = draw_labels_on_map(
            &fonts,
            &[label("top_left", 16f32, &["x"])],
            Some(PlacementSlot::TopLeft),
            &[],
            &mut map2,
        );
        assert_matches!(
            clash,
            Err(Error::BadRequest(_)),
            "legend collision must be rejected"
        );
        Ok(())
    }

    #[test]
    fn blank_label_is_skipped() -> Result<(), Box<dyn std::error::Error>> {
        let fonts = test_fonts()?;
        let mut map = blank_map()?;
        draw_labels_on_map(
            &fonts,
            &[label("center", 16f32, &["", "   "])],
            None,
            &[],
            &mut map,
        )?;
        assert!(!any_pixel_drawn(&map), "an all-blank label draws nothing");
        Ok(())
    }

    #[test]
    fn reserve_rejects_legend_duplicate_and_other_pool() -> Result<(), Box<dyn std::error::Error>> {
        // a slot occupied by the legend is rejected
        let mut used = Vec::new();
        assert_matches!(
            reserve(
                &[PlacementSlot::TopLeft],
                &mut used,
                &[],
                Some(PlacementSlot::TopLeft),
                "label",
            ),
            Err(Error::BadRequest(_))
        );

        // a slot reserved earlier in the same pool is rejected
        let mut used = Vec::new();
        reserve(&[PlacementSlot::TopRight], &mut used, &[], None, "label")?;
        assert_matches!(
            reserve(&[PlacementSlot::TopRight], &mut used, &[], None, "logo"),
            Err(Error::BadRequest(_))
        );

        // a slot reserved by the other pool (e.g. a label) is rejected for a logo
        let mut used = Vec::new();
        assert_matches!(
            reserve(
                &[PlacementSlot::Center],
                &mut used,
                &[PlacementSlot::Center],
                None,
                "logo",
            ),
            Err(Error::BadRequest(_))
        );
        Ok(())
    }

    #[test]
    fn resolve_placement_single_vs_combined() -> Result<(), Box<dyn std::error::Error>> {
        let map = blank_map()?;
        let grid = sl_map_apis::coverage::OccupancyGrid::from_map(
            &map,
            sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
        );
        let slots = grid.evaluate_slots();

        // a single slot reserves just the anchor and yields a positive rect
        let (reserved, rect) = resolve_placement(
            PlacementSlot::TopLeft,
            &[PlacementSlot::TopLeft],
            &slots,
            &grid,
        )?;
        assert_eq!(reserved, vec![PlacementSlot::TopLeft]);
        assert!(rect.width > 0 && rect.height > 0);

        // a combined group reserves exactly the chosen slots and its rectangle
        // spans them (wider than the single top-left slot)
        let group = vec![PlacementSlot::TopLeft, PlacementSlot::TopCenter];
        let (all, combined) = resolve_placement(PlacementSlot::TopLeft, &group, &slots, &grid)?;
        assert_eq!(all, group);
        assert!(combined.width > rect.width);
        Ok(())
    }

    #[test]
    fn parse_slot_group_defaults_and_includes_anchor() -> Result<(), Box<dyn std::error::Error>> {
        use sl_map_apis::coverage::PlacementSlot as P;
        // empty -> just the anchor
        assert_eq!(parse_slot_group(P::TopLeft, &[])?, vec![P::TopLeft]);
        // explicit names, anchor appended if missing, de-duplicated
        let g = parse_slot_group(
            P::TopLeft,
            &["top_center".to_owned(), "top_center".to_owned()],
        )?;
        assert_eq!(g, vec![P::TopCenter, P::TopLeft]);
        assert_matches!(parse_slot_group(P::TopLeft, &["nope".to_owned()]), Err(_));
        Ok(())
    }
}

#[cfg(test)]
mod region_overlay_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn text_overlay_gate_requires_both_size_and_count() {
        // both within bounds -> allowed
        assert!(region_text_overlay_allowed(
            MIN_PIXELS_PER_REGION_FOR_REGION_LABELS,
            MAX_REGIONS_FOR_REGION_LABELS
        ));
        // regions too small -> skipped even with a single region
        assert!(!region_text_overlay_allowed(
            MIN_PIXELS_PER_REGION_FOR_REGION_LABELS - 1.0,
            1
        ));
        // too many regions -> skipped even when each is large
        assert!(!region_text_overlay_allowed(
            256.0,
            MAX_REGIONS_FOR_REGION_LABELS + 1
        ));
        // a comfortably-sized, small render is allowed
        assert!(region_text_overlay_allowed(256.0, 4));
    }

    #[test]
    fn rectangles_write_visible_pixels() -> Result<(), Box<dyn std::error::Error>> {
        // a 4x4 region map at zoom 4 -> 128x128 transparent pixels
        let rect = GridRectangle::new(
            GridCoordinates::new(1000, 1000),
            GridCoordinates::new(1003, 1003),
        );
        let mut map = Map::blank(rect, ZoomLevel::try_new(4)?);
        // before drawing, the map is fully transparent
        let before = (0..map.height())
            .flat_map(|y| (0..map.width()).map(move |x| (x, y)))
            .any(|(x, y)| {
                let image::Rgba([_, _, _, a]) = map.get_pixel(x, y);
                a != 0
            });
        assert!(!before, "blank map should start fully transparent");

        draw_region_rectangles(&mut map);

        // the region grid lines must now have written opaque white pixels
        let mut drawn = false;
        for y in 0..map.height() {
            for x in 0..map.width() {
                if map.get_pixel(x, y) == image::Rgba([255, 255, 255, 255]) {
                    drawn = true;
                }
            }
        }
        assert!(
            drawn,
            "draw_region_rectangles should write opaque white pixels"
        );
        Ok(())
    }

    #[test]
    fn missing_region_fill_paints_the_region_rect() -> Result<(), Box<dyn std::error::Error>> {
        use sl_map_apis::map_tiles::MapLike as _;
        // a 2x2 region map at zoom 4 -> 64x64 px, 32 px per region.
        let rect = GridRectangle::new(
            GridCoordinates::new(1000, 1000),
            GridCoordinates::new(1001, 1001),
        );
        let mut map = Map::blank(rect, ZoomLevel::try_new(4)?);
        // Fill the lower-left region (grid 1000,1000) the way apply_region_overlay
        // does for an `Ok(None)` (missing) region.
        let grid = GridCoordinates::new(1000, 1000);
        let (left, top, width, height) =
            region_pixel_rect(&map, &grid).ok_or("region not in map")?;
        let fill = Rgba([0x19, 0x48, 0x5a, 0xff]);
        map.draw_filled_rect(left, top, width, height, fill);

        // a pixel inside the filled region carries the colour …
        assert_eq!(
            map.get_pixel(left + 1, top + 1),
            fill,
            "fill colour painted"
        );
        // … while a pixel in the diagonally-opposite region stays transparent.
        let other = region_pixel_rect(&map, &GridCoordinates::new(1001, 1001))
            .ok_or("other region not in map")?;
        let image::Rgba([_, _, _, a]) = map.get_pixel(other.0 + 1, other.1 + 1);
        assert_eq!(a, 0, "unfilled region stays transparent");
        Ok(())
    }
}
