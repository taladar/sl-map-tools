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
use image::{ImageFormat, Rgba};
use serde::{Deserialize, Serialize};
use sl_map_apis::map_tiles::{Map, MapProgressEvent, MapTileCache};
use sl_map_apis::region::{RegionNameToGridCoordinatesCache, usb_notecard_to_grid_rectangle};
use sl_types::map::{GridCoordinates, GridRectangle, GridRectangleLike as _, USBNotecard};
use tokio::sync::Mutex;
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
    )
    .await?;
    let (job_id, job) = state.jobs.create_with_id(render_id).await;
    spawn_grid_rectangle_job(state, job_id, job, rect, common);
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
    )
    .await?;

    let (job_id, job) = state.jobs.create_with_id(render_id).await;
    spawn_usb_notecard_job(
        state,
        job_id,
        job,
        notecard,
        parsed.borders,
        parsed.color,
        parsed.common,
        parsed.with_without_route,
    );
    Ok(Json(StartedResponse {
        job_id,
        notecard: Some(notecard_summary),
    }))
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
/// settings JSON. Returns the new render id (same as the caller supplied).
async fn insert_render_row(
    state: &AppState,
    render_id: Uuid,
    destination: Destination,
    created_by: Uuid,
    kind: &str,
    notecard_id: Option<Uuid>,
    settings: &SavedRenderSettings,
) -> Result<(), Error> {
    let (owner_user, owner_group) = match destination {
        Destination::Personal => (Some(created_by.as_bytes().to_vec()), None),
        Destination::Group { group_id } => (None, Some(group_id.as_bytes().to_vec())),
    };
    let now = Utc::now();
    let settings_json = serde_json::to_string(settings)?;
    sqlx::query(
        "INSERT INTO saved_renders \
            (render_id, owner_user_id, owner_group_id, created_by, notecard_id, kind, \
             status, settings_json, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_progress', ?7, ?8)",
    )
    .bind(render_id.as_bytes().to_vec())
    .bind(owner_user)
    .bind(owner_group)
    .bind(created_by.as_bytes().to_vec())
    .bind(notecard_id.map(|id| id.as_bytes().to_vec()))
    .bind(kind)
    .bind(settings_json)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("insert saved_renders failed: {err}");
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
            let result = sqlx::query(
                "UPDATE saved_renders SET status = 'done', finished_at = ?1, \
                    metadata_json = ?2, content_type = ?3, \
                    image_filename = ?4, image_without_route_filename = ?5 \
                 WHERE render_id = ?6",
            )
            .bind(now)
            .bind(metadata_json)
            .bind(*content_type)
            .bind(image_filename)
            .bind(without_filename)
            .bind(render_id.as_bytes().to_vec())
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
) {
    drop(tokio::spawn(async move {
        let result = run_grid_rectangle_job(
            Arc::clone(&state.map_tile_cache),
            Arc::clone(&job),
            rect,
            common,
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
    notecard: USBNotecard,
    borders: (u16, u16, u16, u16),
    route_color: Rgba<u8>,
    common: CommonParams,
    with_without_route: bool,
) {
    drop(tokio::spawn(async move {
        let result = run_usb_notecard_job(
            Arc::clone(&state.map_tile_cache),
            Arc::clone(&state.region_cache),
            Arc::clone(&job),
            notecard,
            borders,
            route_color,
            common,
            with_without_route,
        )
        .await;
        let outcome = finish_job(&job, result).await;
        finalize_render_row(&state, job_id, &outcome).await;
        tracing::info!("render job {job_id} finished");
    }));
}

/// Run the grid-rectangle render to completion.
async fn run_grid_rectangle_job(
    map_tile_cache: Arc<Mutex<MapTileCache>>,
    job: Arc<JobState>,
    rect: GridRectangle,
    common: CommonParams,
) -> Result<JobOutcome, Error> {
    let metadata = build_metadata(&rect);
    let (tx, rx) = tokio::sync::mpsc::channel::<MapProgressEvent>(256);
    let forwarder = tokio::spawn(forward_events(Arc::clone(&job), rx));
    let map = {
        let mut cache = map_tile_cache.lock().await;
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
    let image = encode_map(&map, common.format)?;
    Ok(JobOutcome::Ok {
        image,
        image_without_route: None,
        content_type: common.format.content_type(),
        metadata,
    })
}

/// Run the USB-notecard render to completion.
#[expect(
    clippy::too_many_arguments,
    reason = "this is a one-shot helper invoked from a single spawn site"
)]
async fn run_usb_notecard_job(
    map_tile_cache: Arc<Mutex<MapTileCache>>,
    region_cache: Arc<Mutex<RegionNameToGridCoordinatesCache>>,
    job: Arc<JobState>,
    notecard: USBNotecard,
    borders: (u16, u16, u16, u16),
    route_color: Rgba<u8>,
    common: CommonParams,
    with_without_route: bool,
) -> Result<JobOutcome, Error> {
    let (border_north, border_south, border_east, border_west) = borders;
    let rect = {
        let mut region = region_cache.lock().await;
        usb_notecard_to_grid_rectangle(&mut region, &notecard).await?
    }
    .expanded_west(border_west)
    .expanded_east(border_east)
    .expanded_south(border_south)
    .expanded_north(border_north);
    let metadata = build_metadata(&rect);
    let (tx, rx) = tokio::sync::mpsc::channel::<MapProgressEvent>(256);
    let forwarder = tokio::spawn(forward_events(Arc::clone(&job), rx));
    let (image_without_route, map) = {
        let mut cache = map_tile_cache.lock().await;
        let mut map = Map::new_with_progress(
            &mut cache,
            common.max_width,
            common.max_height,
            rect,
            common.missing_map_tile_color,
            common.missing_region_color,
            Some(&tx),
        )
        .await?;
        let without_route = if with_without_route {
            Some(encode_map(&map, common.format)?)
        } else {
            None
        };
        // release the map tile cache before we lock the region cache for
        // the route to avoid holding both at once
        drop(cache);
        let mut region = region_cache.lock().await;
        map.draw_route_with_progress(&mut region, &notecard, route_color, Some(&tx))
            .await?;
        (without_route, map)
    };
    drop(tx);
    let _join = forwarder.await;
    let image = encode_map(&map, common.format)?;
    Ok(JobOutcome::Ok {
        image,
        image_without_route,
        content_type: common.format.content_type(),
        metadata,
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
