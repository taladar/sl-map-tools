#![doc = include_str!("../../README.md")]

use std::path::PathBuf;

use clap::Parser as _;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use sl_map_apis::map_tiles::{
    Map, MapError, MapProgressEvent, MapTileCache, MapTileCacheError, TileOutcome,
};
use sl_map_apis::region::{
    RegionNameToGridCoordinatesCache, USBNotecardToGridRectangleError,
    usb_notecard_to_grid_rectangle,
};
use sl_types::map::{
    GridCoordinates, GridRectangle, GridRectangleLike as _, USBNotecard, USBNotecardLoadError,
};
use tracing::instrument;
use tracing_subscriber::{
    EnvFilter, Layer as _, Registry, filter::LevelFilter, layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
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
    MapTileCacheError(#[from] Box<MapTileCacheError>),
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
    /// error writing metadata output file
    #[error("error writing metadata output file: {0}")]
    MetadataOutputFileError(#[source] std::io::Error),
    /// the GLW overlay was requested but no --font flag was given
    #[error(
        "GLW overlay requested but no font supplied; pass --font <path-to-ttf> \
         (DejaVuSans.ttf is checked in at the workspace root)"
    )]
    FontRequired,
    /// error reading or writing a file (font, GLW input/output JSON);
    /// `fs_err` includes the offending path in the wrapped message
    #[error("file IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// error parsing the font file as a TrueType font
    #[error("error parsing font file: {0}")]
    FontParseError(#[from] ab_glyph::InvalidFont),
    /// error in the GLW event cache (HTTP fetch / disk cache / JSON)
    #[error("error in GLW event cache: {0}")]
    GlwEventCacheError(#[from] sl_glw::GlwEventCacheError),
    /// error serializing or deserializing a GLW event to/from JSON
    /// (used by --glw-input-file and --glw-output-file)
    #[error("GLW JSON error: {0}")]
    GlwJsonError(#[from] serde_json::Error),
}

impl From<sl_map_apis::text::FontError> for Error {
    fn from(value: sl_map_apis::text::FontError) -> Self {
        match value {
            sl_map_apis::text::FontError::Read { source, .. } => Self::IoError(source),
            sl_map_apis::text::FontError::Parse(invalid) => Self::FontParseError(invalid),
        }
    }
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
    /// the fill color for missing map tiles, default is not to
    /// fill which results in black
    #[clap(long, value_parser = parse_color)]
    pub missing_map_tile_color: Option<image::Rgba<u8>>,
    /// the fill color for missing regions inside higher zoom level map tiles
    /// used, this has some performance impact since we need to determine
    /// if the regions exist, the default if no filling is performed is a color
    /// similar to the water color
    #[clap(long, value_parser = parse_color)]
    pub missing_region_color: Option<image::Rgba<u8>>,
    /// the maximum width of the output file in pixels
    #[clap(long)]
    pub max_width: u32,
    /// the maximum height of the output file in pixels
    #[clap(long)]
    pub max_height: u32,
    /// the output file name for the generated map
    #[clap(long)]
    pub output_file: PathBuf,
    /// optional file to write the metadata (aspect ratio, PPS HUD config) to
    #[clap(long)]
    pub metadata_output_file: Option<PathBuf>,
    /// optional GLW overlay flags; active when --glw-event-id or
    /// --glw-event-key is set
    #[clap(flatten)]
    pub glw: GlwOverlayArgs,
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
        Self::new(
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

/// Optional flag group adding a GLW (GlobalWind) wind/current/wave
/// overlay to a generated map. The group is shared between every
/// map-producing subcommand via `#[clap(flatten)]`. The overlay is
/// active when either `--glw-event-id` or `--glw-event-key` is set; the
/// rest of the flags tune visual style and only take effect in that
/// case.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct GlwOverlayArgs {
    /// numeric GLW event id to overlay. Mutually exclusive with
    /// --glw-event-key and --glw-input-file.
    #[clap(long, conflicts_with_all = ["glw_event_key", "glw_input_file"])]
    pub glw_event_id: Option<u32>,
    /// string GLW event key to overlay. Mutually exclusive with
    /// --glw-event-id and --glw-input-file.
    #[clap(long, conflicts_with_all = ["glw_event_id", "glw_input_file"])]
    pub glw_event_key: Option<String>,
    /// path to a JSON file containing a previously-fetched GLW event
    /// (use the file written by --glw-output-file). When set, no HTTP
    /// fetch is performed and --glw-base-url is ignored. Mutually
    /// exclusive with --glw-event-id and --glw-event-key.
    #[clap(long, conflicts_with_all = ["glw_event_id", "glw_event_key"])]
    pub glw_input_file: Option<PathBuf>,
    /// optional path to write the fetched (or loaded) GLW event to as
    /// pretty-printed JSON. Useful for offline reruns: fetch once with
    /// --glw-event-id and --glw-output-file foo.json, then on later
    /// runs pass --glw-input-file foo.json to skip the network.
    #[clap(long)]
    pub glw_output_file: Option<PathBuf>,
    /// override the GLW base URL (including the `glw127`/version path
    /// segment). Defaults to the workspace default
    /// (<http://globalwind.net/glw127/>). Ignored when --glw-input-file
    /// is set.
    #[clap(long)]
    pub glw_base_url: Option<url::Url>,
    /// draw the dashed margin band marking each override's
    /// blending-zone outer boundary. Off by default.
    #[clap(long)]
    pub glw_margin_band: bool,
    /// hex colour for area rectangle outlines (e.g. `#28dc28`).
    #[clap(long, value_parser = parse_color)]
    pub glw_area_outline_color: Option<image::Rgba<u8>>,
    /// hex colour for circle outlines.
    #[clap(long, value_parser = parse_color)]
    pub glw_circle_outline_color: Option<image::Rgba<u8>>,
    /// hex colour for the dashed margin band.
    #[clap(long, value_parser = parse_color)]
    pub glw_margin_outline_color: Option<image::Rgba<u8>>,
    /// hex colour for filled wind arrows.
    #[clap(long, value_parser = parse_color)]
    pub glw_wind_color: Option<image::Rgba<u8>>,
    /// hex colour for filled current arrows.
    #[clap(long, value_parser = parse_color)]
    pub glw_current_color: Option<image::Rgba<u8>>,
    /// hex colour for wave glyph strokes.
    #[clap(long, value_parser = parse_color)]
    pub glw_wave_color: Option<image::Rgba<u8>>,
    /// hex colour to fill area interiors with. When unset (the
    /// default), interiors stay transparent.
    #[clap(long, value_parser = parse_color)]
    pub glw_area_fill_color: Option<image::Rgba<u8>>,
}

/// Internal resolution of the three mutually-exclusive GLW
/// event-source flags.
enum GlwSource {
    /// Event identified by its numeric id; fetched via the cache.
    ById(sl_glw::EventId),
    /// Event identified by its string key; fetched via the cache.
    ByKey(sl_glw::GlwEventKey),
    /// Event loaded from a JSON file on disk (no network).
    FromFile(PathBuf),
}

impl GlwOverlayArgs {
    /// Resolve the user-supplied source flags into a typed
    /// [`GlwSource`], or `None` if no GLW overlay was requested.
    fn source(&self) -> Option<GlwSource> {
        if let Some(id) = self.glw_event_id {
            return Some(GlwSource::ById(sl_glw::EventId::new(id)));
        }
        if let Some(key) = self.glw_event_key.as_deref() {
            return Some(GlwSource::ByKey(sl_glw::GlwEventKey::new(key)));
        }
        if let Some(path) = self.glw_input_file.as_ref() {
            return Some(GlwSource::FromFile(path.clone()));
        }
        None
    }

    /// Build a [`sl_glw::GlwStyle`] from the library default, then
    /// apply any CLI overrides the user supplied.
    fn build_style(&self) -> sl_glw::GlwStyle {
        let mut style = sl_glw::GlwStyle {
            legend_position: Some(sl_map_apis::coverage::PlacementSlot::TopLeft),
            draw_margin_band: self.glw_margin_band,
            ..sl_glw::GlwStyle::default()
        };
        if let Some(c) = self.glw_area_outline_color {
            style.palette.area_outline = c;
        }
        if let Some(c) = self.glw_circle_outline_color {
            style.palette.circle_outline = c;
        }
        if let Some(c) = self.glw_margin_outline_color {
            style.palette.margin_outline = c;
        }
        if let Some(c) = self.glw_wind_color {
            style.palette.wind_arrow = c;
        }
        if let Some(c) = self.glw_current_color {
            style.palette.current_arrow = c;
        }
        if let Some(c) = self.glw_wave_color {
            style.palette.wave_glyph = c;
        }
        if let Some(c) = self.glw_area_fill_color {
            style.palette.area_fill = Some(c);
        }
        style
    }
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
    /// number of extra regions of border to add on every side of the
    /// rectangle derived from the USB notecard waypoints. Cannot be
    /// combined with the per-direction --border-{north,south,east,west}
    /// flags.
    #[clap(
        long,
        conflicts_with_all = ["border_north", "border_south", "border_east", "border_west"],
    )]
    pub border_regions: Option<u16>,
    /// extra regions of border to add on the north (+y) side of the
    /// rectangle derived from the USB notecard waypoints
    #[clap(long)]
    pub border_north: Option<u16>,
    /// extra regions of border to add on the south (-y) side of the
    /// rectangle derived from the USB notecard waypoints
    #[clap(long)]
    pub border_south: Option<u16>,
    /// extra regions of border to add on the east (+x) side of the
    /// rectangle derived from the USB notecard waypoints
    #[clap(long)]
    pub border_east: Option<u16>,
    /// extra regions of border to add on the west (-x) side of the
    /// rectangle derived from the USB notecard waypoints
    #[clap(long)]
    pub border_west: Option<u16>,
    /// the fill color for missing map tiles, default is not to
    /// fill which results in black
    #[clap(long, value_parser = parse_color)]
    pub missing_map_tile_color: Option<image::Rgba<u8>>,
    /// the fill color for missing regions inside higher zoom level map tiles
    /// used, this has some performance impact since we need to determine
    /// if the regions exist, the default if no filling is performed is a color
    /// similar to the water color
    #[clap(long, value_parser = parse_color)]
    pub missing_region_color: Option<image::Rgba<u8>>,
    /// the maximum width of the output file in pixels
    #[clap(long)]
    pub max_width: u32,
    /// the maximum height of the output file in pixels
    #[clap(long)]
    pub max_height: u32,
    /// the output file name for the generated map without the route
    #[clap(long)]
    pub output_file_without_route: Option<PathBuf>,
    /// the output file name for the generated map
    #[clap(long)]
    pub output_file: PathBuf,
    /// optional file to write the metadata (aspect ratio, PPS HUD config) to
    #[clap(long)]
    pub metadata_output_file: Option<PathBuf>,
    /// optional GLW overlay flags; active when --glw-event-id or
    /// --glw-event-key is set
    #[clap(flatten)]
    pub glw: GlwOverlayArgs,
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
    /// path to a TrueType font used for any text rendering on the map
    /// (currently the GLW labels and corner legend). Required only when
    /// a feature that draws text is requested; at present that means any
    /// GLW overlay. Plain map renders work without this flag. The
    /// workspace does not bundle a default font; a copy of
    /// `DejaVuSans.ttf` is checked in at the workspace root for
    /// convenience.
    #[clap(long)]
    font: Option<PathBuf>,
    /// which subcommand to use
    #[clap(subcommand)]
    command: Command,
}

/// Print metadata (PPS HUD config, aspect ratio) to stdout and optionally to a file
///
/// # Errors
///
/// returns an error when writing to the metadata output file fails
#[expect(
    clippy::result_large_err,
    reason = "this is only used once at the end of the run so we only return from it once"
)]
fn output_metadata(
    grid_rectangle: &GridRectangle,
    metadata_output_file: Option<&PathBuf>,
) -> Result<(), crate::Error> {
    let pps_config = format!("PPS HUD config: {}", grid_rectangle.pps_hud_config());
    let aspect = format!(
        "The aspect ratio of the image is {}:{} ({})",
        grid_rectangle.size_x(),
        grid_rectangle.size_y(),
        f32::from(grid_rectangle.size_x()) / f32::from(grid_rectangle.size_y())
    );
    let note =
        "You can use this to edit e.g. the PPS HUD to have the correct ratio of width and height";
    println!("{pps_config}");
    println!("{aspect}");
    println!("{note}");
    if let Some(path) = metadata_output_file {
        fs_err::write(path, format!("{pps_config}\n{aspect}\n{note}\n"))
            .map_err(crate::Error::MetadataOutputFileError)?;
    }
    Ok(())
}

/// Drain `MapProgressEvent`s from the channel and drive `indicatif`
/// progress bars on stderr, one per phase that the renderer reports
/// (tile fetch, optional region existence check, optional route waypoint
/// resolution). Returns once the sender side of the channel is dropped,
/// so the caller can drop its `tx`, await this task, and be sure every
/// event has been displayed before continuing.
///
/// `indicatif` auto-detects non-TTY stderr and silently hides the bars,
/// so this is safe to call unconditionally.
async fn report_progress(mut rx: tokio::sync::mpsc::Receiver<MapProgressEvent>) {
    let multi = MultiProgress::new();
    let bar_style = ProgressStyle::with_template(
        "{prefix:<14} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {msg}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar())
    .progress_chars("=>-");

    let mut tiles: Option<ProgressBar> = None;
    let mut regions: Option<ProgressBar> = None;
    let mut waypoints: Option<ProgressBar> = None;
    let mut region_names: Option<ProgressBar> = None;
    let mut memory_hits: u64 = 0;
    let mut disk_hits: u64 = 0;
    let mut network_fetches: u64 = 0;
    let mut missing_tiles: u64 = 0;

    while let Some(event) = rx.recv().await {
        match event {
            MapProgressEvent::PlanComputed {
                zoom_level,
                total_tiles,
            } => {
                let pb = multi.add(ProgressBar::new(u64::from(total_tiles)));
                pb.set_style(bar_style.to_owned());
                pb.set_prefix(format!("tiles z={}", zoom_level.into_inner()));
                tiles = Some(pb);
            }
            MapProgressEvent::TileStarted { .. } => {}
            MapProgressEvent::TileFinished { outcome, .. } => {
                match outcome {
                    TileOutcome::LoadedFromMemoryCache => {
                        memory_hits = memory_hits.saturating_add(1);
                    }
                    TileOutcome::LoadedFromDiskCache => {
                        disk_hits = disk_hits.saturating_add(1);
                    }
                    TileOutcome::FetchedFromNetwork => {
                        network_fetches = network_fetches.saturating_add(1);
                    }
                    TileOutcome::Missing => {
                        missing_tiles = missing_tiles.saturating_add(1);
                    }
                }
                if let Some(pb) = tiles.as_ref() {
                    pb.inc(1);
                    pb.set_message(format!(
                        "mem={memory_hits} disk={disk_hits} net={network_fetches} missing={missing_tiles}"
                    ));
                }
            }
            MapProgressEvent::RegionCheckPlanned { total_regions } => {
                let pb = multi.add(ProgressBar::new(u64::from(total_regions)));
                pb.set_style(bar_style.to_owned());
                pb.set_prefix("regions");
                regions = Some(pb);
            }
            MapProgressEvent::RegionChecked { .. } => {
                if let Some(pb) = regions.as_ref() {
                    pb.inc(1);
                }
            }
            MapProgressEvent::RoutePlanned { total_waypoints } => {
                let total = u64::try_from(total_waypoints).unwrap_or(u64::MAX);
                let pb = multi.add(ProgressBar::new(total));
                pb.set_style(bar_style.to_owned());
                pb.set_prefix("waypoints");
                waypoints = Some(pb);
            }
            MapProgressEvent::RouteWaypointResolved { region, .. } => {
                if let Some(pb) = waypoints.as_ref() {
                    pb.inc(1);
                    pb.set_message(region.into_inner());
                }
            }
            MapProgressEvent::RegionNamesPlanned { total_regions } => {
                let pb = multi.add(ProgressBar::new(u64::from(total_regions)));
                pb.set_style(bar_style.to_owned());
                pb.set_prefix("region names");
                region_names = Some(pb);
            }
            MapProgressEvent::RegionNameResolved { .. } => {
                if let Some(pb) = region_names.as_ref() {
                    pb.inc(1);
                }
            }
        }
    }

    if let Some(pb) = tiles {
        pb.finish();
    }
    if let Some(pb) = regions {
        pb.finish();
    }
    if let Some(pb) = waypoints {
        pb.finish();
    }
}

/// Await the progress task, logging a warning if it panicked. Used so the
/// caller can drop the sender, wait for the receiver to drain, and then
/// continue without a `must_use` lint on the join handle's result.
async fn join_progress_task(handle: tokio::task::JoinHandle<()>) {
    if let Err(err) = handle.await {
        tracing::warn!("progress task did not finish cleanly: {err}");
    }
}

/// If `args` requests a GLW overlay, obtain the event (via the
/// three-tier HTTP cache rooted at `cache_dir`, or by reading a JSON
/// file on disk when `--glw-input-file` is set), load the supplied
/// font, optionally write the event back out to `--glw-output-file`,
/// and draw the overlay onto `map`. Returns `Ok(true)` when an overlay
/// was actually drawn, `Ok(false)` when no GLW source was requested or
/// the server returned no event for the requested id/key. Returns
/// [`Error::FontRequired`] if a source was requested but `font_path`
/// is `None`.
async fn fetch_and_draw_glw(
    cache_dir: &std::path::Path,
    font_path: Option<&std::path::Path>,
    args: &GlwOverlayArgs,
    map: &mut Map,
) -> Result<bool, crate::Error> {
    use sl_glw::MapLikeGlwExt as _;
    let Some(source) = args.source() else {
        return Ok(false);
    };
    let Some(font_path) = font_path else {
        return Err(crate::Error::FontRequired);
    };
    let font = sl_map_apis::text::load_font(font_path)?;

    let event: Option<sl_glw::GlwEvent> = match source {
        GlwSource::ById(id) => {
            let mut cache =
                sl_glw::GlwEventCache::new(cache_dir.to_owned(), args.glw_base_url.clone())?;
            cache.get_event_by_id(id).await?
        }
        GlwSource::ByKey(key) => {
            let mut cache =
                sl_glw::GlwEventCache::new(cache_dir.to_owned(), args.glw_base_url.clone())?;
            cache.get_event_by_key(&key).await?
        }
        GlwSource::FromFile(path) => {
            let json = fs_err::read_to_string(&path)?;
            Some(serde_json::from_str::<sl_glw::GlwEvent>(&json)?)
        }
    };
    let Some(event) = event else {
        tracing::warn!(
            "GLW event not found (server returned no event); rendering map without overlay"
        );
        return Ok(false);
    };

    // Pretty-print the event back out before drawing, so even partial
    // runs (drawing might fail later) leave the user with the fetched
    // JSON for inspection or offline reruns.
    if let Some(output_path) = args.glw_output_file.as_ref() {
        let json = serde_json::to_string_pretty(&event)?;
        fs_err::write(output_path, json)?;
    }

    let style = args.build_style();
    map.draw_glw_event_with_font(&event, &style, &font);
    Ok(true)
}

/// The main behaviour of the binary should go here
#[instrument]
async fn do_stuff() -> Result<(), crate::Error> {
    let options = Options::parse();
    tracing::debug!("{:#?}", options);

    let Options {
        cache_dir,
        font,
        command,
    } = options;

    match command {
        Command::FromGridRectangle(from_grid_rectangle) => {
            let ratelimiter = ratelimit::Ratelimiter::builder(10).build()?;
            let mut map_tile_cache = MapTileCache::new(cache_dir.clone(), Some(ratelimiter));
            let grid_rectangle: GridRectangle = (&from_grid_rectangle).into();
            let (tx, rx) = tokio::sync::mpsc::channel::<MapProgressEvent>(256);
            let progress_task = tokio::spawn(report_progress(rx));
            let mut map = Map::new_with_progress(
                &mut map_tile_cache,
                from_grid_rectangle.max_width,
                from_grid_rectangle.max_height,
                grid_rectangle.to_owned(),
                from_grid_rectangle.missing_map_tile_color,
                from_grid_rectangle.missing_region_color,
                Some(&tx),
            )
            .await?;
            drop(tx);
            join_progress_task(progress_task).await;
            // GLW overlay (optional) is drawn after the base map and
            // before save. No route to layer above it in this branch.
            fetch_and_draw_glw(
                &cache_dir,
                font.as_deref(),
                &from_grid_rectangle.glw,
                &mut map,
            )
            .await?;
            map.save(&from_grid_rectangle.output_file)?;
            output_metadata(
                &grid_rectangle,
                from_grid_rectangle.metadata_output_file.as_ref(),
            )?;
        }
        Command::FromUSBNotecard(from_usb_notecard) => {
            let usb_notecard = USBNotecard::load_from_file(&from_usb_notecard.usb_notecard)?;
            let mut region_name_to_grid_coordinates_cache =
                RegionNameToGridCoordinatesCache::new(cache_dir.clone())?;
            let (border_north, border_south, border_east, border_west) =
                if let Some(b) = from_usb_notecard.border_regions {
                    (b, b, b, b)
                } else {
                    (
                        from_usb_notecard.border_north.unwrap_or(0),
                        from_usb_notecard.border_south.unwrap_or(0),
                        from_usb_notecard.border_east.unwrap_or(0),
                        from_usb_notecard.border_west.unwrap_or(0),
                    )
                };
            let grid_rectangle = usb_notecard_to_grid_rectangle(
                &mut region_name_to_grid_coordinates_cache,
                &usb_notecard,
            )
            .await?
            .expanded_west(border_west)
            .expanded_east(border_east)
            .expanded_south(border_south)
            .expanded_north(border_north);
            let ratelimiter = ratelimit::Ratelimiter::builder(10).build()?;
            let mut map_tile_cache = MapTileCache::new(cache_dir.clone(), Some(ratelimiter));
            let (tx, rx) = tokio::sync::mpsc::channel::<MapProgressEvent>(256);
            let progress_task = tokio::spawn(report_progress(rx));
            let mut map = Map::new_with_progress(
                &mut map_tile_cache,
                from_usb_notecard.max_width,
                from_usb_notecard.max_height,
                grid_rectangle.to_owned(),
                from_usb_notecard.missing_map_tile_color,
                from_usb_notecard.missing_region_color,
                Some(&tx),
            )
            .await?;
            // Optional no-overlay-no-route diagnostic save happens
            // BEFORE any overlays are drawn so the file reflects only
            // the base map.
            if let Some(output_file_without_route) = &from_usb_notecard.output_file_without_route {
                map.save(output_file_without_route)?;
            }
            // Layering: base map → GLW overlay → route on top → save.
            // GLW happens here so the route line stays the most-readable
            // element of the final image.
            fetch_and_draw_glw(
                &cache_dir,
                font.as_deref(),
                &from_usb_notecard.glw,
                &mut map,
            )
            .await?;
            map.draw_route_with_progress(
                &mut region_name_to_grid_coordinates_cache,
                &usb_notecard,
                from_usb_notecard.color,
                Some(&tx),
            )
            .await?;
            drop(tx);
            join_progress_task(progress_task).await;
            map.save(&from_usb_notecard.output_file)?;
            output_metadata(
                &grid_rectangle,
                from_usb_notecard.metadata_output_file.as_ref(),
            )?;
        }
    }

    Ok(())
}

/// The main function mainly just handles setting up tracing
/// and handling any Err Results.
#[tokio::main]
#[expect(
    clippy::result_large_err,
    reason = "this is main so we only return from it once"
)]
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
        Ok(()) => (),
        Err(e) => {
            tracing::error!("{}", e);
            eprintln!("{e}");
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
