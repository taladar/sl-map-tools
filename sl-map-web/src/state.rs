//! Shared application state passed to every handler.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use sl_map_apis::map_tiles::MapTileCache;
use sl_map_apis::region::RegionNameToGridCoordinatesCache;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::jobs::JobStore;

/// State shared between all axum handlers.
///
/// The two cache types both require `&mut self` to drive lookups, so they
/// are wrapped in async [`Mutex`]es. As a side effect this serialises
/// renders to one at a time; that is acceptable for v1 because each render
/// is also bounded by the upstream rate limiter.
#[derive(Clone, Debug)]
#[expect(
    clippy::module_name_repetitions,
    reason = "`AppState` is the conventional name for axum shared state and would be ambiguous as just `State`"
)]
pub struct AppState {
    /// the on-disk + in-memory cache of map tiles, shared with the CLI.
    pub map_tile_cache: Arc<Mutex<MapTileCache>>,
    /// the region name <-> grid coordinates cache.
    pub region_cache: Arc<Mutex<RegionNameToGridCoordinatesCache>>,
    /// the in-memory store of running and recently completed render jobs.
    pub jobs: Arc<JobStore>,
    /// the runtime configuration.
    pub config: Arc<Config>,
    /// SQLite pool used by the auth subsystem (users, sessions, set-password
    /// tokens). Cheap to clone.
    pub db: sqlx::SqlitePool,
    /// signing key for the session cookie, derived from the configured
    /// `session_signing_key` at startup. `Key` is internally a buffer of
    /// bytes; cloning is cheap.
    pub cookie_key: Key,
    /// in-process flag raised whenever a code path may have produced stale
    /// files under `<storage_dir>/renders/` without unlinking them inline
    /// (e.g. a cascade delete from removing a group). The orphan sweeper
    /// only scans the filesystem when this flag is set so an idle server
    /// does no filesystem work.
    pub library_cleanup_dirty: Arc<AtomicBool>,
}

// `axum_extra::extract::cookie::SignedCookieJar` extracts itself via
// `FromRef<S, Key>`, so we expose the `cookie_key` through `FromRef`.
impl FromRef<AppState> for Key {
    fn from_ref(input: &AppState) -> Self {
        input.cookie_key.clone()
    }
}
