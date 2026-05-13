//! Render endpoints: start a render job and return its id.

use std::io::Cursor;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Multipart, State};
use bytes::Bytes;
use image::{ImageFormat, Rgba};
use serde::{Deserialize, Serialize};
use sl_map_apis::map_tiles::{Map, MapProgressEvent, MapTileCache};
use sl_map_apis::region::{RegionNameToGridCoordinatesCache, usb_notecard_to_grid_rectangle};
use sl_types::map::{GridCoordinates, GridRectangle, GridRectangleLike as _, USBNotecard};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::Error;
use crate::jobs::{JobId, JobOutcome, JobState, Metadata, ProgressDto, record_event};
use crate::state::AppState;

/// Output format for the rendered image.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// PNG output (default).
    #[default]
    Png,
    /// JPEG output.
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
}

/// Response shape for both render endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct StartedResponse {
    /// the id of the newly created job.
    pub job_id: Uuid,
}

/// `POST /api/render/grid-rectangle` — start a render from explicit corners.
///
/// # Errors
///
/// Returns an error if any of the optional hex colour fields fails to parse;
/// otherwise the request always succeeds (the actual render runs in the
/// background; failures there are surfaced via the SSE stream and the
/// result endpoints).
pub async fn grid_rectangle(
    State(state): State<AppState>,
    Json(req): Json<GridRectangleRequest>,
) -> Result<Json<StartedResponse>, Error> {
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
    let rect = GridRectangle::new(
        GridCoordinates::new(req.lower_left_x, req.lower_left_y),
        GridCoordinates::new(req.upper_right_x, req.upper_right_y),
    );
    let (job_id, job) = state.jobs.create().await;
    spawn_grid_rectangle_job(state, job_id, job, rect, common);
    Ok(Json(StartedResponse { job_id }))
}

/// `POST /api/render/usb-notecard` — start a render from a notecard.
///
/// # Errors
///
/// Returns an error if the multipart form is malformed, the notecard fails
/// to parse, or required fields are missing.
pub async fn usb_notecard(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<StartedResponse>, Error> {
    let (notecard, borders, color, common, with_without_route) =
        parse_render_form(multipart).await?;
    let (job_id, job) = state.jobs.create().await;
    spawn_usb_notecard_job(
        state,
        job_id,
        job,
        notecard,
        borders,
        color,
        common,
        with_without_route,
    );
    Ok(Json(StartedResponse { job_id }))
}

/// Parse a hex colour string (e.g. `#ff0000`) into an `image::Rgba<u8>`.
fn parse_color(s: &str) -> Result<Rgba<u8>, Error> {
    let parsed = hex_color::HexColor::parse(s)
        .map_err(|e| Error::BadRequest(format!("invalid colour `{s}`: {e}")))?;
    Ok(Rgba(parsed.to_be_bytes()))
}

/// Parse the multipart form for the USB-notecard render endpoint. Returns
/// the notecard, per-side borders, route colour, shared params, and whether
/// the caller asked to keep a no-route version.
async fn parse_render_form(
    multipart: Multipart,
) -> Result<
    (
        USBNotecard,
        (u16, u16, u16, u16),
        Rgba<u8>,
        CommonParams,
        bool,
    ),
    Error,
> {
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
            _ => {}
        }
    }
    let raw = notecard_file.or(notecard_text).ok_or_else(|| {
        Error::BadRequest(
            "either a `notecard` file upload or a `notecard_text` field is required".to_owned(),
        )
    })?;
    let notecard: USBNotecard = raw.parse()?;
    let max_width = max_width.ok_or_else(|| Error::BadRequest("missing max_width".to_owned()))?;
    let max_height =
        max_height.ok_or_else(|| Error::BadRequest("missing max_height".to_owned()))?;
    let borders = form.borders();
    Ok((
        notecard,
        borders,
        color,
        CommonParams {
            max_width,
            max_height,
            missing_map_tile_color,
            missing_region_color,
            format,
        },
        with_without_route,
    ))
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
        finish_job(&job, result).await;
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
        finish_job(&job, result).await;
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
async fn finish_job(job: &Arc<JobState>, result: Result<JobOutcome, Error>) {
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
    // `send_replace` (rather than `send`) stores the value even when no
    // receivers exist yet, so the result endpoints can still serve the
    // outcome when an SSE subscriber connects after the render finishes.
    drop(job.outcome.send_replace(Some(Arc::new(outcome))));
}
