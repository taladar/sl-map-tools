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
use sl_types::map::{GridCoordinates, GridRectangle, GridRectangleLike as _, USBNotecard};
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
/// buffer, so 32 768 caps a single render at ~4 GiB on the extreme edge
/// while leaving plenty of headroom for any realistic map. Beyond a sanity
/// check it prevents an attacker-supplied `max_width` / `max_height` from
/// driving the server out of memory.
const MAX_OUTPUT_DIMENSION: u32 = 0x8000;

/// Maximum number of in-progress renders per user. The renderer is
/// serialised on a single map-tile cache, so one user submitting many
/// concurrent jobs would block all other users. Three is a small ceiling
/// that lets a user kick off a couple of variants in parallel without
/// monopolising the worker.
const MAX_CONCURRENT_RENDERS_PER_USER: i64 = 3;

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
    /// optional hex colour to fill area interiors with; when `None`
    /// interiors are transparent.
    #[serde(default)]
    pub area_fill_color: Option<String>,
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
    /// GLW overlay used for the render. When set, the carrier is always
    /// `GlwSource::SavedId` so `Regenerate` reliably points at a row in
    /// `saved_glw_data` instead of refetching from the GLW server.
    #[serde(default)]
    pub glw: Option<GlwRenderOptions>,
    /// free-floating text labels drawn on the render.
    #[serde(default)]
    pub labels: Vec<TextLabel>,
}

/// Persisted form fields for a USB-notecard render.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// whether a without-route variant was also produced.
    pub save_without_route: bool,
    /// GLW overlay used for the render. See [`SavedGridRectangleSettings::glw`].
    #[serde(default)]
    pub glw: Option<GlwRenderOptions>,
    /// free-floating text labels drawn on the render.
    #[serde(default)]
    pub labels: Vec<TextLabel>,
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
    };
    let destination = Destination::parse(req.save_to.as_deref().unwrap_or("personal"))?;
    library::assert_can_write(&state.db, user.user_id, destination).await?;
    assert_under_concurrent_limit(&state.db, user.user_id).await?;
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
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
        // The worker may rewrite this in place after a successful
        // fresh-fetch so the carrier always lands as `SavedId` at rest
        // (stable Regenerate).
        glw: req.glw.clone(),
        labels: req.labels.clone(),
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
    let (job_id, job) = state.jobs.create_with_id(render_id).await;
    let glw_ctx = req.glw.clone().map(|opts| GlwJobCtx {
        options: opts,
        destination,
        created_by: user.user_id,
    });
    spawn_grid_rectangle_job(state, job_id, job, rect, common, glw_ctx, req.labels);
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
    assert_under_concurrent_limit(&state.db, user.user_id).await?;

    // Resolve the notecard: reuse an existing one (auto-copied into the
    // render's scope if needed) or persist a freshly uploaded one. The
    // returned summary is what the response surfaces back to the UI so it
    // can update its dropdown if the effective id is new.
    let (notecard, notecard_summary) = resolve_notecard(&state, &user, &parsed).await?;

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
        save_without_route: parsed.with_without_route,
        // The worker may rewrite this in place after a successful
        // fresh-fetch so the carrier always lands as `SavedId` at rest.
        glw: parsed.glw.clone(),
        labels: parsed.labels.clone(),
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

    let (job_id, job) = state.jobs.create_with_id(render_id).await;
    let glw_ctx = parsed.glw.clone().map(|opts| GlwJobCtx {
        options: opts,
        destination: parsed.destination,
        created_by: user.user_id,
    });
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
    /// the largest empty rectangle that can be placed anchored here, or
    /// `null` when the anchor is covered.
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

/// Response shape for the `placement-slots` endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct PlacementSlotsResponse {
    /// width in pixels of the image the final render will produce.
    pub image_width: u32,
    /// height in pixels of the image the final render will produce.
    pub image_height: u32,
    /// the nine candidate anchors, in reading order.
    pub slots: Vec<SlotDto>,
}

/// Reduce a drawn-on blank map to the nine-slot placement report.
fn compute_placement_slots(map: &Map) -> PlacementSlotsResponse {
    use sl_map_apis::coverage::PlacementSlot;
    let image = sl_map_apis::map_tiles::MapLike::image(map);
    let (image_width, image_height) = image.dimensions();
    let slots = sl_map_apis::coverage::OccupancyGrid::from_map(
        map,
        sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
    )
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
    PlacementSlotsResponse {
        image_width,
        image_height,
        slots,
    }
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
    let mut map = Map::blank_fit(rect, req.max_width, req.max_height)?;
    if let Some(glw) = req.glw.as_ref() {
        apply_glw_overlay_readonly(&state, user.user_id, glw, &mut map).await?;
    }
    Ok(Json(compute_placement_slots(&map)))
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
    Ok(Json(compute_placement_slots(&map)))
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
    let mut notecard_text: Option<String> = None;
    let mut notecard_file: Option<String> = None;
    let mut notecard_id: Option<Uuid> = None;
    let mut destination_raw: Option<String> = None;
    let mut notecard_name: Option<String> = None;
    let mut glw: Option<GlwRenderOptions> = None;
    let mut labels: Vec<TextLabel> = Vec::new();
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
                with_without_route = matches!(
                    raw.trim().to_ascii_lowercase().as_str(),
                    "1" | "on" | "true" | "yes"
                );
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
fn spawn_grid_rectangle_job(
    state: AppState,
    job_id: JobId,
    job: Arc<JobState>,
    rect: GridRectangle,
    common: CommonParams,
    glw_ctx: Option<GlwJobCtx>,
    labels: Vec<TextLabel>,
) {
    drop(tokio::spawn(async move {
        let result = run_grid_rectangle_job(
            state.clone(),
            Arc::clone(&job),
            rect,
            common,
            glw_ctx,
            labels,
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
        )
        .await;
        let outcome = finish_job(&job, result).await;
        finalize_render_row(&state, job_id, &outcome).await;
        tracing::info!("render job {job_id} finished");
    }));
}

/// Run the grid-rectangle render to completion.
async fn run_grid_rectangle_job(
    state: AppState,
    job: Arc<JobState>,
    rect: GridRectangle,
    common: CommonParams,
    glw_ctx: Option<GlwJobCtx>,
    labels: Vec<TextLabel>,
) -> Result<JobOutcome, Error> {
    let metadata = build_metadata(&rect);
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
    drop(tx);
    // wait for the forwarder so the event history is complete before we
    // signal completion to subscribers
    let _join = forwarder.await;
    // Layering: base map below, GLW overlay on top. No route for the
    // grid-rectangle path. Labels go last, above everything.
    let glw_data_id = apply_glw_overlay_to_map(&state, glw_ctx.as_ref(), &mut map).await?;
    draw_labels_on_map(
        &state.fonts,
        &labels,
        legend_slot_of(glw_ctx.as_ref()),
        &mut map,
    )?;
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
) -> Result<JobOutcome, Error> {
    let (border_north, border_south, border_east, border_west) = borders;
    let bare_rect = {
        let mut region = state.region_cache.lock().await;
        usb_notecard_to_grid_rectangle(&mut region, &notecard).await?
    };
    // Cache the bare rectangle (without border padding) on the notecard
    // row so the library UI can show its bounds without redoing region
    // resolution. Done before render-side expansion to keep notecard
    // bounds == route's bounding box.
    update_notecard_bounds(&state.db, notecard_id, &bare_rect).await?;
    let rect = bare_rect
        .expanded_west(border_west)
        .expanded_east(border_east)
        .expanded_south(border_south)
        .expanded_north(border_north);
    // Backfill the bounds on the saved_renders row now that we know the
    // rectangle; the library UI shows them even for `in_progress` rows.
    update_render_bounds(&state.db, render_id, &rect).await?;
    let metadata = build_metadata(&rect);
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
        // GLW overlay sits between the base map and the route, so the
        // route line stays the most-readable element of the final
        // image.
        let glw_data_id = apply_glw_overlay_to_map(&state, glw_ctx.as_ref(), &mut map).await?;
        {
            let mut region = state.region_cache.lock().await;
            map.draw_route_with_progress(&mut region, &notecard, route_color, Some(&tx))
                .await?;
        }
        // Labels go last, above the route.
        draw_labels_on_map(
            &state.fonts,
            &labels,
            legend_slot_of(glw_ctx.as_ref()),
            &mut map,
        )?;
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
    if let Some(c) = overrides.area_fill_color.as_deref() {
        style.palette.area_fill = Some(parse_color(c.trim())?);
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

/// Draw the free-floating text labels onto `map`, last (above the route and
/// GLW overlay). Rejects (as a `BadRequest`) any label that overflows its
/// slot's free space, two labels sharing a slot, or a label sharing the
/// legend's slot — these mirror the client-side checks and are the
/// authoritative fallback. Each label is aligned within its slot's free
/// rectangle using its own alignment, defaulting to the slot's outward
/// alignment.
fn draw_labels_on_map(
    fonts: &crate::fonts::FontDirectory,
    labels: &[TextLabel],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    map: &mut Map,
) -> Result<(), Error> {
    use sl_map_apis::map_tiles::MapLike as _;
    if labels.is_empty() {
        return Ok(());
    }
    // Resolve every label's slot and reject collisions before drawing.
    let resolved_slots: Vec<sl_map_apis::coverage::PlacementSlot> = labels
        .iter()
        .map(|label| {
            label
                .slot
                .parse()
                .map_err(|err: sl_map_apis::coverage::ParsePlacementSlotError| {
                    Error::BadRequest(err.to_string())
                })
        })
        .collect::<Result<_, _>>()?;
    let mut used: Vec<sl_map_apis::coverage::PlacementSlot> = Vec::new();
    for anchor in &resolved_slots {
        if legend_slot == Some(*anchor) {
            return Err(Error::BadRequest(format!(
                "a label uses slot `{anchor}` which is occupied by the legend"
            )));
        }
        if used.contains(anchor) {
            return Err(Error::BadRequest(format!(
                "two labels target the same slot `{anchor}`"
            )));
        }
        used.push(*anchor);
    }
    // Authoritative free space for each slot, measured on the current map.
    let slots = sl_map_apis::coverage::OccupancyGrid::from_map(
        map,
        sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
    )
    .evaluate_slots();
    for (label, anchor) in labels.iter().zip(resolved_slots.iter()) {
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
        let font_path = fonts
            .path_for(&label.font_id)
            .ok_or_else(|| Error::BadRequest(format!("unknown font_id `{}`", label.font_id)))?;
        let font = sl_map_apis::text::load_font(font_path)?;
        let color = parse_color(label.color.trim())?;
        let scale = ab_glyph::PxScale::from(label.font_px);
        let (text_w, text_h) = sl_map_apis::text::measure_text(scale, &font, &lines);
        let slot = slots
            .iter()
            .find(|info| info.slot == *anchor)
            .ok_or_else(|| Error::BadRequest(format!("slot `{anchor}` not found")))?;
        let Some(rect) = slot.free_rect else {
            return Err(Error::BadRequest(format!(
                "slot `{anchor}` is fully covered; no room for a label"
            )));
        };
        if text_w > rect.width || text_h > rect.height {
            return Err(Error::BadRequest(format!(
                "label text renders at {text_w}x{text_h} px but slot `{anchor}` only has {}x{} px free",
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
        let style = sl_map_apis::text::LabelStyle {
            scale,
            fg: color,
            shadow: Rgba([0, 0, 0, 180]),
        };
        map.draw_text_label(
            (
                i32::try_from(origin_x).unwrap_or(0),
                i32::try_from(origin_y).unwrap_or(0),
            ),
            &lines,
            &style,
            &font,
        );
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
    use sl_types::map::ZoomLevel;

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
        let resp = compute_placement_slots(&map);
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
        let resp = compute_placement_slots(&map);
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
}

#[cfg(test)]
mod label_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use pretty_assertions::assert_matches;
    use sl_map_apis::coverage::{HAlign, PlacementSlot, VAlign};
    use sl_types::map::ZoomLevel;

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
        draw_labels_on_map(&fonts, &[label("top_left", 18f32, &["Hi"])], None, &mut map)?;
        assert!(any_pixel_drawn(&map), "a fitting label should draw pixels");

        // a font far too large to fit the 352px map is rejected
        let mut map2 = blank_map()?;
        let err = draw_labels_on_map(
            &fonts,
            &[label("center", 4000f32, &["BIG"])],
            None,
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
            &mut map,
        )?;
        assert!(!any_pixel_drawn(&map), "an all-blank label draws nothing");
        Ok(())
    }
}
