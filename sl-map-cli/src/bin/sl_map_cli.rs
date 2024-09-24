#![doc = include_str!("../../README.md")]

use std::path::PathBuf;

use clap::Parser;

use sl_map_apis::map_tiles::{Map, MapError, MapLike, MapTileCache, MapTileCacheError};
use sl_map_apis::region::{
    usb_notecard_to_grid_rectangle, RegionNameToGridCoordinatesCache,
    USBNotecardToGridRectangleError,
};
use sl_types::map::{
    GridCoordinates, GridRectangle, RegionName, USBNotecard, USBNotecardLoadError,
};
use tracing::instrument;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, Registry,
};

/// Error enum for the application
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// error reading environment variable
    #[error("error when retrieving environment variable: {0}")]
    EnvVarError(#[from] std::env::VarError),
    /// error in clap
    #[error("error in CLI option parsing: {0}")]
    ClapError(#[from] clap::Error),
    /// error parsing log filter
    #[error("error parsing log filter: {0}")]
    LogFilterParseError(#[from] tracing_subscriber::filter::ParseError),
    /// error in ratelimiter for downloads
    #[error("error in ratelimiter: {0}")]
    RateLimiterError(#[from] ratelimit::Error),
    /// error in map tile cache
    #[error("error in map tile cache: {0}")]
    MapTileCacheError(#[from] MapTileCacheError),
    /// error in map generation
    #[error("error in map generation: {0}")]
    MapError(#[from] MapError),
    /// error in image processing
    #[error("error in image processing: {0}")]
    ImageError(#[from] image::error::ImageError),
    /// error loading USB notecard
    #[error("error loading USB notecard: {0}")]
    USBNotecardLoadError(#[from] USBNotecardLoadError),
    /// region name/grid coordinate cache error
    #[error("error in region name/grid coordinate cache: {0}")]
    RegionNameCacheError(#[from] sl_map_apis::region::CacheError),
    /// error converting a USB notecard to a grid rectangle
    #[error("error converting a USB notecard to a grid rectangle: {0}")]
    USBNotecardToGridRectangleError(#[from] USBNotecardToGridRectangleError),
    /// no grid coordinates were returned for one of the region names in the
    /// USB Notecard
    #[error("No grid coordinates were returned for one of the regions in the USB notecard: {0}")]
    NoGridCoordinatesForRegion(RegionName),
    /// error converting grid and region coordinates into pixel coordinates on a map
    #[error("error converting grid and region coordinates into pixel coordinates on a map")]
    MapCoordinateError,
}

/// Generate a map from a rectangle of grid coordinates
#[derive(clap::Parser, Debug, Clone)]
pub struct FromGridRectangle {
    /// the x coordinate of the lower left corner of the grid rectangle
    #[clap(long)]
    pub lower_left_x: u16,
    /// the y coordinate of the lower left corner of the grid rectangle
    #[clap(long)]
    pub lower_left_y: u16,
    /// the x coordinate of the upper right corner of the grid rectangle
    #[clap(long)]
    pub upper_right_x: u16,
    /// the y coordinate of the upper right corner of the grid rectangle
    #[clap(long)]
    pub upper_right_y: u16,
    /// the maximum width of the output file in pixels
    #[clap(long)]
    pub max_width: u32,
    /// the maximum height of the output file in pixels
    #[clap(long)]
    pub max_height: u32,
    /// the output file name for the generated map
    #[clap(long)]
    pub output_file: PathBuf,
}

impl From<&FromGridRectangle> for GridRectangle {
    fn from(
        &FromGridRectangle {
            lower_left_x,
            lower_left_y,
            upper_right_x,
            upper_right_y,
            ..
        }: &FromGridRectangle,
    ) -> Self {
        GridRectangle::new(
            GridCoordinates::new(lower_left_x.to_owned(), lower_left_y.to_owned()),
            GridCoordinates::new(upper_right_x.to_owned(), upper_right_y.to_owned()),
        )
    }
}

/// parse `image::Rgba` from a hex color string
///
/// # Errors
///
/// fails if the string could not be parsed as a hex color
pub fn parse_color(s: &str) -> Result<image::Rgba<u8>, hex_color::ParseHexColorError> {
    let hex_color = hex_color::HexColor::parse(s)?;
    Ok(image::Rgba(hex_color.to_be_bytes()))
}

/// Generate a map from a USB notecard
#[derive(clap::Parser, Debug, Clone)]
pub struct FromUSBNotecard {
    /// the filename for the USB notecard file
    #[clap(long)]
    pub usb_notecard: PathBuf,
    /// the color to use for the waypoints and route
    #[clap(long, value_parser = parse_color, default_value = "#f00")]
    pub color: image::Rgba<u8>,
    /// the maximum width of the output file in pixels
    #[clap(long)]
    pub max_width: u32,
    /// the maximum height of the output file in pixels
    #[clap(long)]
    pub max_height: u32,
    /// the output file name for the generated map
    #[clap(long)]
    pub output_file: PathBuf,
}

/// which subcommand to call
#[derive(clap::Parser, Debug)]
pub enum Command {
    /// Generate a map from a rectangle of grid coordinates
    FromGridRectangle(FromGridRectangle),
    /// Generate a map from a USB notecard
    FromUSBNotecard(FromUSBNotecard),
}

/// The Clap type for all the commandline parameters
#[derive(clap::Parser, Debug)]
#[clap(name = clap::crate_name!(),
       about = clap::crate_description!(),
       author = clap::crate_authors!(),
       version = clap::crate_version!(),
       )]
struct Options {
    /// cache dir for map tiles
    #[clap(long)]
    cache_dir: PathBuf,
    /// which subcommand to use
    #[clap(subcommand)]
    command: Command,
}

/// The main behaviour of the binary should go here
#[instrument]
async fn do_stuff() -> Result<(), crate::Error> {
    let options = Options::parse();
    tracing::debug!("{:#?}", options);

    match options.command {
        Command::FromGridRectangle(from_grid_rectangle) => {
            let ratelimiter =
                ratelimit::Ratelimiter::builder(1, std::time::Duration::from_millis(100))
                    .build()?;
            let mut map_tile_cache = MapTileCache::new(options.cache_dir, Some(ratelimiter));
            let map = Map::new(
                &mut map_tile_cache,
                from_grid_rectangle.max_width,
                from_grid_rectangle.max_height,
                (&from_grid_rectangle).into(),
            )
            .await?;
            map.save(&from_grid_rectangle.output_file)?;
        }
        Command::FromUSBNotecard(from_usb_notecard) => {
            let usb_notecard = USBNotecard::load_from_file(&from_usb_notecard.usb_notecard)?;
            let region_name_to_grid_coordinates_cache = RegionNameToGridCoordinatesCache::new(
                options.cache_dir.to_owned(),
                std::time::Duration::from_secs(7 * 24 * 60 * 60),
            )?;
            let grid_rectangle = usb_notecard_to_grid_rectangle(
                &region_name_to_grid_coordinates_cache,
                &usb_notecard,
            )
            .await?;
            let ratelimiter =
                ratelimit::Ratelimiter::builder(1, std::time::Duration::from_millis(100))
                    .build()?;
            let mut map_tile_cache = MapTileCache::new(options.cache_dir, Some(ratelimiter));
            let mut map = Map::new(
                &mut map_tile_cache,
                from_usb_notecard.max_width,
                from_usb_notecard.max_height,
                grid_rectangle,
            )
            .await?;
            let mut previous_waypoint = None;
            for waypoint in usb_notecard.waypoints() {
                let Some(grid_coordinates) = region_name_to_grid_coordinates_cache
                    .get_grid_coordinates(waypoint.location().region_name())
                    .await?
                else {
                    return Err(crate::Error::NoGridCoordinatesForRegion(
                        waypoint.location().region_name().to_owned(),
                    ));
                };
                let (x, y) = map
                    .pixel_coordinates_for_coordinates(
                        &grid_coordinates,
                        &waypoint.region_coordinates(),
                    )
                    .ok_or(crate::Error::MapCoordinateError)?;
                tracing::debug!("Drawing waypoint at ({x}, {y})");
                map.draw_waypoint(x, y, from_usb_notecard.color);
                if let Some((previous_x, previous_y)) = previous_waypoint {
                    tracing::debug!("Drawing line from ({previous_x}, {previous_y}) to ({x}, {y})");
                    map.draw_line(previous_x, previous_y, x, y, from_usb_notecard.color);
                }
                previous_waypoint = Some((x, y));
            }
            map.save(&from_usb_notecard.output_file)?;
        }
    }

    Ok(())
}

/// The main function mainly just handles setting up tracing
/// and handling any Err Results.
#[tokio::main]
async fn main() -> Result<(), Error> {
    let terminal_env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .parse(std::env::var("RUST_LOG").unwrap_or_else(|_| "".to_string()))?;
    let file_env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::TRACE.into())
        .parse(std::env::var("SL_MAP_CLI_LOG").unwrap_or_else(|_| "".to_string()))?;
    let registry = Registry::default();
    let registry =
        registry.with(tracing_subscriber::fmt::Layer::default().with_filter(terminal_env_filter));
    let log_dir = std::env::var("SL_MAP_CLI_LOG_DIR");
    let file_layer = if let Ok(log_dir) = log_dir {
        let log_file = if let Ok(log_file) = std::env::var("SL_MAP_CLI_LOG_FILE") {
            log_file
        } else {
            "sl_map_cli.log".to_string()
        };
        tracing::info!("Logging to {}/{}", log_dir, log_file);
        let file_appender = tracing_appender::rolling::never(log_dir, log_file);
        Some(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(file_appender)
                .with_filter(file_env_filter),
        )
    } else {
        None
    };
    let registry = registry.with(file_layer);
    registry.init();
    log_panics::init();
    match do_stuff().await {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("{}", e);
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
    tracing::debug!("Exiting");
    Ok(())
}

#[cfg(test)]
mod test {
    //use super::*;
    //use pretty_assertions::{assert_eq, assert_ne};
}
