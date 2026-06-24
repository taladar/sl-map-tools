//! HTTP handler for `POST /api/region/lookup`.
//!
//! Resolves a region name to its grid coordinates using the same cached
//! lookup the USB notecard resolution uses
//! (`RegionNameToGridCoordinatesCache::get_grid_coordinates`). The grid
//! rectangle search field calls this to set both rectangle corners to a named
//! region.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use sl_types::map::RegionName;

use crate::auth::CurrentUser;
use crate::error::Error;
use crate::state::AppState;

/// Request body for `POST /api/region/lookup`.
#[derive(Debug, Deserialize)]
pub struct LookupRequest {
    /// the region name to resolve.
    pub region_name: String,
}

/// Response body for `POST /api/region/lookup`.
#[derive(Debug, Serialize)]
pub struct LookupResponse {
    /// resolved x grid coordinate of the region.
    pub x: u32,
    /// resolved y grid coordinate of the region.
    pub y: u32,
}

/// `POST /api/region/lookup` — resolve a region name to its grid coordinates.
///
/// # Errors
///
/// Returns `BadRequest` if the region name fails validation (it must be
/// 2–35 characters), `NotFound` if no region with that name exists, or a
/// region cache error if the upstream lookup fails.
pub async fn lookup(
    _user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<LookupRequest>,
) -> Result<Json<LookupResponse>, Error> {
    let region_name = RegionName::try_new(req.region_name)
        .map_err(|e| Error::BadRequest(format!("invalid region name: {e}")))?;
    let mut region_cache = state.region_cache.lock().await;
    let coords = region_cache.get_grid_coordinates(&region_name).await?;
    drop(region_cache);
    let coords = coords
        .ok_or_else(|| Error::NotFound(format!("no region found with name `{region_name}`")))?;
    Ok(Json(LookupResponse {
        x: coords.x(),
        y: coords.y(),
    }))
}
