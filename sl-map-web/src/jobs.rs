//! In-memory store of render jobs and the per-job state used by handlers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sl_map_apis::map_tiles::{MapProgressEvent, TileOutcome};
use sl_types::map::MapTileDescriptor;
use tokio::sync::{Mutex, broadcast, watch};
use uuid::Uuid;

/// Unique identifier for a render job.
pub type JobId = Uuid;

/// Serializable view of a [`MapProgressEvent`] for the SSE stream.
///
/// The `MapProgressEvent` from `sl-map-apis` is not `Serialize`; this DTO
/// translates each variant into a JSON-friendly shape.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressDto {
    /// the rendering plan has been computed.
    PlanComputed {
        /// chosen zoom level for the render.
        zoom_level: u8,
        /// total number of tiles that will be fetched / processed.
        total_tiles: u32,
    },
    /// processing of a tile has started.
    TileStarted {
        /// zoom level of the tile.
        zoom: u8,
        /// lower-left x grid coordinate of the tile.
        x: u32,
        /// lower-left y grid coordinate of the tile.
        y: u32,
    },
    /// processing of a tile has finished.
    TileFinished {
        /// zoom level of the tile.
        zoom: u8,
        /// lower-left x grid coordinate of the tile.
        x: u32,
        /// lower-left y grid coordinate of the tile.
        y: u32,
        /// where the tile came from (memory, disk, network, missing).
        outcome: &'static str,
    },
    /// the renderer will check per-region existence (triggered by the
    /// `missing_region_color` option). Often slower than the main tile
    /// fetch because each check may pull several higher-zoom tiles.
    RegionCheckPlanned {
        /// upper bound on the number of region checks.
        total_regions: u32,
    },
    /// a region's existence has been determined.
    RegionChecked {
        /// x grid coordinate of the region.
        x: u32,
        /// y grid coordinate of the region.
        y: u32,
        /// whether the region exists.
        exists: bool,
    },
    /// the route drawing phase has started.
    RoutePlanned {
        /// number of waypoints in the route.
        total_waypoints: usize,
    },
    /// a waypoint in the route has been resolved.
    RouteWaypointResolved {
        /// 0-based index of the waypoint.
        index: usize,
        /// total number of waypoints.
        total: usize,
        /// resolved region name.
        region: String,
    },
    /// resolving region names for the per-region annotation overlay has
    /// started.
    RegionNamesPlanned {
        /// number of regions whose names will be resolved.
        total_regions: u32,
    },
    /// one region's name has been resolved for the overlay.
    RegionNameResolved {
        /// 0-based index of the region.
        index: u32,
        /// total number of regions whose names will be resolved.
        total: u32,
    },
    /// the job has finished successfully.
    Done,
    /// the job has finished with an error.
    Error {
        /// human-readable error message.
        message: String,
    },
}

impl From<MapProgressEvent> for ProgressDto {
    fn from(value: MapProgressEvent) -> Self {
        match value {
            MapProgressEvent::PlanComputed {
                zoom_level,
                total_tiles,
            } => Self::PlanComputed {
                zoom_level: zoom_level.into_inner(),
                total_tiles,
            },
            MapProgressEvent::TileStarted { descriptor } => {
                let (zoom, x, y) = split_descriptor(&descriptor);
                Self::TileStarted { zoom, x, y }
            }
            MapProgressEvent::TileFinished {
                descriptor,
                outcome,
            } => {
                let (zoom, x, y) = split_descriptor(&descriptor);
                Self::TileFinished {
                    zoom,
                    x,
                    y,
                    outcome: outcome_str(outcome),
                }
            }
            MapProgressEvent::RegionCheckPlanned { total_regions } => {
                Self::RegionCheckPlanned { total_regions }
            }
            MapProgressEvent::RegionChecked { x, y, exists } => {
                Self::RegionChecked { x, y, exists }
            }
            MapProgressEvent::RoutePlanned { total_waypoints } => {
                Self::RoutePlanned { total_waypoints }
            }
            MapProgressEvent::RouteWaypointResolved {
                index,
                total,
                region,
            } => Self::RouteWaypointResolved {
                index,
                total,
                region: region.into_inner(),
            },
            MapProgressEvent::RegionNamesPlanned { total_regions } => {
                Self::RegionNamesPlanned { total_regions }
            }
            MapProgressEvent::RegionNameResolved { index, total } => {
                Self::RegionNameResolved { index, total }
            }
        }
    }
}

/// Extract `(zoom, x, y)` from a `MapTileDescriptor` for JSON output.
fn split_descriptor(descriptor: &MapTileDescriptor) -> (u8, u32, u32) {
    let zoom = (*descriptor.zoom_level()).into_inner();
    let corner = descriptor.lower_left_corner();
    (zoom, corner.x(), corner.y())
}

/// Map a [`TileOutcome`] to a short stable string for the JSON payload.
const fn outcome_str(outcome: TileOutcome) -> &'static str {
    match outcome {
        TileOutcome::LoadedFromMemoryCache => "memory",
        TileOutcome::LoadedFromDiskCache => "disk",
        TileOutcome::FetchedFromNetwork => "network",
        TileOutcome::Missing => "missing",
    }
}

/// Metadata returned alongside a finished render (mirrors what the CLI
/// prints to stdout / writes to the metadata file).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// the x size of the rendered grid rectangle (in regions).
    pub aspect_x: u32,
    /// the y size of the rendered grid rectangle (in regions).
    pub aspect_y: u32,
    /// width / height as a float.
    pub aspect_ratio: f64,
    /// the PPS HUD config string (suitable for the dot prim description).
    pub pps_hud_config: String,
}

/// The terminal state of a job once the render task has finished.
#[derive(Debug, Clone)]
pub enum JobOutcome {
    /// the render succeeded; the bytes and metadata are stored in memory.
    Ok {
        /// the encoded primary image.
        image: Bytes,
        /// optional encoded image without the route overlay.
        image_without_route: Option<Bytes>,
        /// the MIME type matching the encoded format.
        content_type: &'static str,
        /// metadata about the render (aspect ratio, PPS HUD config).
        metadata: Metadata,
        /// id of the `saved_glw_data` row referenced by the render, if a
        /// GLW overlay was actually drawn (None when no GLW was
        /// requested or the server returned no event).
        glw_data_id: Option<uuid::Uuid>,
    },
    /// the render failed.
    Err(String),
}

/// Per-job runtime state.
#[derive(Debug)]
pub struct JobState {
    /// instant the job was created (used for TTL eviction).
    pub created: Instant,
    /// recorded progress events; the SSE handler reads from this and the
    /// render task appends to it as events arrive on the mpsc channel.
    pub events: Mutex<Vec<ProgressDto>>,
    /// broadcast "ping" channel used to wake up SSE handlers; the actual
    /// event content is read from `events`, this is just a signal.
    pub ping: broadcast::Sender<()>,
    /// the terminal outcome, set once the render task finishes.
    pub outcome: watch::Sender<Option<Arc<JobOutcome>>>,
}

impl JobState {
    /// Create a fresh job state with empty buffers and a `None` outcome.
    #[must_use]
    pub fn new() -> Self {
        // a generous broadcast capacity so that a slow SSE handler does
        // not lose pings; the actual events live in `events` so a lag here
        // just causes a re-poll, not data loss.
        let (ping, _) = broadcast::channel(64);
        let (outcome, _) = watch::channel(None);
        Self {
            created: Instant::now(),
            events: Mutex::new(Vec::new()),
            ping,
            outcome,
        }
    }
}

impl Default for JobState {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory map of job ids to job state.
#[derive(Debug, Default)]
pub struct JobStore {
    /// inner map; mutex-wrapped so handlers can mutate concurrently.
    inner: Mutex<HashMap<JobId, Arc<JobState>>>,
}

impl JobStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new job, returning its id and shared state handle.
    pub async fn create(&self) -> (JobId, Arc<JobState>) {
        self.create_with_id(Uuid::new_v4()).await
    }

    /// Register a new job using a caller-supplied id. Used so the persisted
    /// `saved_renders.render_id` and the in-memory job id are the same UUID,
    /// keeping the existing `/api/render/{id}/...` endpoints addressing the
    /// same render as the new `/api/renders/{id}/...` ones.
    pub async fn create_with_id(&self, id: JobId) -> (JobId, Arc<JobState>) {
        let state = Arc::new(JobState::new());
        {
            let mut guard = self.inner.lock().await;
            drop(guard.insert(id, Arc::clone(&state)));
        }
        (id, state)
    }

    /// Look up the job state for a given id.
    pub async fn get(&self, id: JobId) -> Option<Arc<JobState>> {
        let guard = self.inner.lock().await;
        guard.get(&id).map(Arc::clone)
    }

    /// Evict all jobs whose age exceeds `max_age` and which have already
    /// finished. Running jobs are never evicted so an in-flight render can
    /// complete.
    pub async fn evict_older_than(&self, max_age: Duration) {
        let now = Instant::now();
        let mut guard = self.inner.lock().await;
        guard.retain(|_, state| {
            let age = now.saturating_duration_since(state.created);
            let finished = state.outcome.borrow().is_some();
            !finished || age <= max_age
        });
    }
}

/// Helper for the render task: push an event into the job's history and
/// signal the SSE handlers.
pub async fn record_event(state: &JobState, event: ProgressDto) {
    {
        let mut events = state.events.lock().await;
        events.push(event);
    }
    // best-effort: no receivers is fine, that just means no-one is
    // listening for live updates yet.
    drop(state.ping.send(()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// The region-name overlay progress events must serialise to the exact
    /// `type` tags and field names the client's `handleProgress` switch reads.
    #[test]
    fn region_name_progress_events_serialise_for_the_client()
    -> Result<(), Box<dyn std::error::Error>> {
        let planned = ProgressDto::from(MapProgressEvent::RegionNamesPlanned { total_regions: 42 });
        assert_eq!(
            serde_json::to_value(&planned)?,
            serde_json::json!({ "type": "region_names_planned", "total_regions": 42 })
        );

        let resolved = ProgressDto::from(MapProgressEvent::RegionNameResolved {
            index: 7,
            total: 42,
        });
        assert_eq!(
            serde_json::to_value(&resolved)?,
            serde_json::json!({ "type": "region_name_resolved", "index": 7, "total": 42 })
        );
        Ok(())
    }
}
