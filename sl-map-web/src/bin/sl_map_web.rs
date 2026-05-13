#![doc = include_str!("../../README.md")]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum_extra::extract::cookie::Key;
use clap::Parser as _;
use sl_map_apis::map_tiles::MapTileCache;
use sl_map_apis::region::RegionNameToGridCoordinatesCache;
use sl_map_web::auth::run_cleanup;
use sl_map_web::config::{Config, ConfigError};
use sl_map_web::db::{DbError, open_and_migrate};
use sl_map_web::jobs::JobStore;
use sl_map_web::routes::build as build_router;
use sl_map_web::state::AppState;
use tokio::sync::Mutex;
use tracing_subscriber::{
    EnvFilter, Layer as _, Registry, filter::LevelFilter, layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};

/// Top-level error type for the binary.
#[derive(thiserror::Error, Debug)]
enum Error {
    /// error parsing a `tracing-subscriber` filter directive.
    #[error("error parsing log filter: {0}")]
    LogFilter(#[from] tracing_subscriber::filter::ParseError),
    /// configuration validation failed (missing required field, bad
    /// signing key, etc.). Startup aborts.
    #[error("invalid configuration: {0}")]
    Config(#[from] ConfigError),
    /// error building the ratelimiter.
    #[error("error in ratelimiter: {0}")]
    RateLimiter(#[from] ratelimit::Error),
    /// error opening the region name cache.
    #[error("error opening region name cache: {0}")]
    RegionCache(#[from] sl_map_apis::region::CacheError),
    /// error opening the auth SQLite database or applying migrations.
    #[error("auth database error: {0}")]
    Db(#[from] DbError),
    /// error from the axum HTTP listener.
    #[error("HTTP listener error: {0}")]
    Listener(#[source] std::io::Error),
    /// error running the axum server.
    #[error("HTTP server error: {0}")]
    Server(#[source] std::io::Error),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    install_tracing()?;
    log_panics::init();
    if let Err(err) = run().await {
        tracing::error!("{err}");
        eprintln!("{err}");
        std::process::exit(1);
    }
    Ok(())
}

/// Wire up the application state, build the router, and serve it.
async fn run() -> Result<(), Error> {
    let config = Config::parse();
    config.validate()?;
    tracing::info!(bind = %config.bind, "starting sl-map-web");
    if !config.cache_dir.exists() {
        fs_err::create_dir_all(&config.cache_dir).map_err(Error::Listener)?;
    }
    let ratelimiter = ratelimit::Ratelimiter::builder(config.rate_limit).build()?;
    let map_tile_cache = MapTileCache::new(config.cache_dir.clone(), Some(ratelimiter));
    let region_cache = RegionNameToGridCoordinatesCache::new(config.cache_dir.clone())?;
    let jobs = Arc::new(JobStore::new());
    let job_ttl = Duration::from_secs(config.job_ttl_seconds);

    let db = open_and_migrate(&config.database_url).await?;
    // validated already; safe to decode again here.
    let signing_bytes = config.decoded_signing_key()?;
    let cookie_key = Key::from(signing_bytes.as_slice());

    let state = AppState {
        map_tile_cache: Arc::new(Mutex::new(map_tile_cache)),
        region_cache: Arc::new(Mutex::new(region_cache)),
        jobs: Arc::clone(&jobs),
        config: Arc::new(config.clone()),
        db: db.clone(),
        cookie_key,
    };
    spawn_job_evictor(jobs, job_ttl);
    spawn_auth_cleanup(db);
    let router = build_router(state);
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(Error::Listener)?;
    tracing::info!("listening on {}", config.bind);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(Error::Server)?;
    Ok(())
}

/// Background task that periodically prunes finished jobs older than the
/// configured TTL.
fn spawn_job_evictor(jobs: Arc<JobStore>, max_age: Duration) {
    drop(tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            jobs.evict_older_than(max_age).await;
        }
    }));
}

/// Background task that prunes expired sessions and set-password tokens.
fn spawn_auth_cleanup(pool: sqlx::SqlitePool) {
    drop(tokio::spawn(async move {
        run_cleanup(pool).await;
    }));
}

/// Install the same dual-sink tracing setup the CLI uses (env-filterable
/// terminal output plus an optional rolling file).
fn install_tracing() -> Result<(), Error> {
    let terminal_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse(std::env::var("RUST_LOG").unwrap_or_default())?;
    let file_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::TRACE.into())
        .parse(std::env::var("SL_MAP_WEB_LOG").unwrap_or_default())?;
    let registry = Registry::default()
        .with(tracing_subscriber::fmt::Layer::default().with_filter(terminal_filter));
    let file_layer = if let Ok(log_dir) = std::env::var("SL_MAP_WEB_LOG_DIR") {
        let log_file =
            std::env::var("SL_MAP_WEB_LOG_FILE").unwrap_or_else(|_| "sl_map_web.log".to_owned());
        tracing::info!("logging to {log_dir}/{log_file}");
        let appender = tracing_appender::rolling::never(log_dir, log_file);
        Some(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(appender)
                .with_filter(file_filter),
        )
    } else {
        None
    };
    registry.with(file_layer).init();
    Ok(())
}
