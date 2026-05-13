//! Runtime configuration for the `sl-map-web` server.

use std::net::SocketAddr;
use std::path::PathBuf;

/// CLI options for `sl_map_web`.
///
/// Each field may also be supplied via the matching `SL_MAP_WEB_*`
/// environment variable. Explicit CLI flags override the env vars.
#[derive(Debug, Clone, clap::Parser)]
#[clap(
    name = clap::crate_name!(),
    about = clap::crate_description!(),
    author = clap::crate_authors!(),
    version = clap::crate_version!(),
)]
pub struct Config {
    /// cache directory shared with the CLI; the same redb / jpg / cache
    /// policy files are used.
    #[clap(long, env = "SL_MAP_WEB_CACHE_DIR")]
    pub cache_dir: PathBuf,

    /// socket address to bind the HTTP server to.
    #[clap(long, env = "SL_MAP_WEB_BIND", default_value = "127.0.0.1:3000")]
    pub bind: SocketAddr,

    /// rate limit for upstream tile requests (requests per second).
    #[clap(long, env = "SL_MAP_WEB_RATE_LIMIT", default_value_t = 10)]
    pub rate_limit: u64,

    /// time-to-live (seconds) for completed render jobs in memory.
    #[clap(long, env = "SL_MAP_WEB_JOB_TTL", default_value_t = 600)]
    pub job_ttl_seconds: u64,
}
