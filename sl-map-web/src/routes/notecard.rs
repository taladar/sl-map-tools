//! Handler that resolves a USB notecard to a grid rectangle (for client-side
//! preview rendering).

use axum::Json;
use axum::extract::{Multipart, State};
use serde::Serialize;
use sl_map_apis::region::usb_notecard_to_grid_rectangle;
use sl_types::map::{GridRectangleLike as _, USBNotecard};

use crate::auth::CurrentUser;
use crate::error::Error;
use crate::library;
use crate::state::AppState;
use crate::usb_notecard::parse_notecard_form;

/// One waypoint with its resolved grid coordinates.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedWaypoint {
    /// the region the waypoint refers to.
    pub region_name: String,
    /// resolved x grid coordinate of the region.
    pub region_x: u16,
    /// resolved y grid coordinate of the region.
    pub region_y: u16,
    /// in-region x coordinate of the waypoint (metres).
    pub x: f32,
    /// in-region y coordinate of the waypoint (metres).
    pub y: f32,
    /// in-region z coordinate of the waypoint (metres).
    pub z: f32,
}

/// Response shape for `/api/notecard/derive-rectangle`.
#[derive(Debug, Clone, Serialize)]
pub struct DeriveResponse {
    /// lower-left x grid coordinate (after border expansion).
    pub lower_left_x: u16,
    /// lower-left y grid coordinate (after border expansion).
    pub lower_left_y: u16,
    /// upper-right x grid coordinate (after border expansion).
    pub upper_right_x: u16,
    /// upper-right y grid coordinate (after border expansion).
    pub upper_right_y: u16,
    /// the resolved waypoints.
    pub waypoints: Vec<ResolvedWaypoint>,
}

/// `POST /api/notecard/derive-rectangle` — parse the uploaded/pasted notecard
/// and return the bounding rectangle (with optional border expansion) plus
/// the resolved waypoints, suitable for a client-side preview overlay.
///
/// # Errors
///
/// Returns an error if the multipart form is malformed, the notecard
/// cannot be parsed, or a referenced region cannot be resolved.
pub async fn derive_rectangle(
    user: CurrentUser,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<DeriveResponse>, Error> {
    let form = parse_notecard_form(multipart).await?;
    let (border_north, border_south, border_east, border_west) = form.borders();
    let notecard = match (form.notecard, form.notecard_id) {
        (Some(nc), _) => nc,
        (None, Some(id)) => {
            // The same auth gate the library / render endpoints use — a
            // caller who cannot read the notecard gets the indistinguishable
            // NotFound, never the body.
            let row = library::assert_can_read_notecard(&state.db, user.user_id, id).await?;
            row.body.parse()?
        }
        (None, None) => return Err(Error::BadRequest("missing notecard".to_owned())),
    };
    let mut region_cache = state.region_cache.lock().await;
    let rect = usb_notecard_to_grid_rectangle(&mut region_cache, &notecard)
        .await?
        .expanded_west(border_west)
        .expanded_east(border_east)
        .expanded_south(border_south)
        .expanded_north(border_north);
    let waypoints = resolve_waypoints(&mut region_cache, &notecard).await?;
    drop(region_cache);
    let lower_left = rect.lower_left_corner();
    let upper_right = rect.upper_right_corner();
    Ok(Json(DeriveResponse {
        lower_left_x: lower_left.x(),
        lower_left_y: lower_left.y(),
        upper_right_x: upper_right.x(),
        upper_right_y: upper_right.y(),
        waypoints,
    }))
}

/// Resolve each waypoint's region to grid coordinates. Returns an error if
/// any region cannot be resolved.
async fn resolve_waypoints(
    region_cache: &mut sl_map_apis::region::RegionNameToGridCoordinatesCache,
    notecard: &USBNotecard,
) -> Result<Vec<ResolvedWaypoint>, Error> {
    let mut out = Vec::with_capacity(notecard.waypoints().len());
    for waypoint in notecard.waypoints() {
        let region = waypoint.location().region_name();
        let grid = region_cache
            .get_grid_coordinates(region)
            .await?
            .ok_or_else(|| {
                Error::BadRequest(format!(
                    "could not resolve region `{}` to grid coordinates",
                    region.to_owned().into_inner()
                ))
            })?;
        let rc = waypoint.region_coordinates();
        out.push(ResolvedWaypoint {
            region_name: region.to_owned().into_inner(),
            region_x: grid.x(),
            region_y: grid.y(),
            x: rc.x(),
            y: rc.y(),
            z: rc.z(),
        });
    }
    Ok(out)
}
