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
    /// error in map generation (boxed: `MapError` is large and this keeps the
    /// aggregate `Error` small enough to pass by value cheaply)
    #[error("error in map generation: {0}")]
    MapError(Box<MapError>),
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
    /// a text feature (region names/coordinates, or a text label) was requested
    /// but no font is available for it; the message names the feature
    #[error(
        "{0} requested but no font supplied; pass --font <path-to-ttf> (or a \
         per-feature font path) — DejaVuSans.ttf is checked in at the workspace root"
    )]
    TextFontRequired(&'static str),
    /// a logo or label placement could not be satisfied (bad slot/alignment,
    /// non-rectangular combined slot, overflow of the free area, or a slot clash)
    #[error("placement error: {0}")]
    Placement(String),
    /// the requested output dimensions are outside the allowed bounds
    #[error("invalid output dimensions: {0}")]
    InvalidDimensions(String),
    /// a subcommand that needs the map-tile cache was run without --cache-dir
    #[error("this subcommand requires --cache-dir <path>")]
    CacheDirRequired,
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

impl From<MapError> for Error {
    fn from(value: MapError) -> Self {
        Self::MapError(Box::new(value))
    }
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
    pub lower_left_x: u32,
    /// the y coordinate of the lower left corner of the grid rectangle
    #[clap(long)]
    pub lower_left_y: u32,
    /// the x coordinate of the upper right corner of the grid rectangle
    #[clap(long)]
    pub upper_right_x: u32,
    /// the y coordinate of the upper right corner of the grid rectangle
    #[clap(long)]
    pub upper_right_y: u32,
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
    /// optional region-overlay, output-format and logo/label placement flags
    #[clap(flatten)]
    pub placement: PlacementArgs,
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

/// Output image format for a generated map. Mirrors the web app's
/// `OutputFormat`; `png` is the default and `jpeg` (alias `jpg`) is the
/// lossy alternative.
#[derive(clap::ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// PNG output (lossless, default)
    #[default]
    Png,
    /// JPEG output (lossy, smaller files)
    #[clap(alias = "jpg")]
    Jpeg,
}

impl OutputFormat {
    /// the matching `image::ImageFormat` for encoding
    const fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Jpeg => image::ImageFormat::Jpeg,
        }
    }
}

/// default scale (native size) for a logo placement whose JSON omits `scale`
const fn default_logo_scale() -> u8 {
    1
}

/// default colour (opaque white) for a text label whose JSON omits `color`
fn default_label_color() -> String {
    "#ffffff".to_owned()
}

/// A logo image to composite onto the map, supplied as one JSON object per
/// `--logo` flag. `slot` is one of the nine placement-slot names (e.g.
/// `bottom_right`), or several joined with `+` to combine them into one larger
/// rectangle (the combined slots must form a solid rectangle, e.g.
/// `bottom_left+bottom_center`).
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LogoSpec {
    /// path to the logo image file (PNG, JPEG or WebP)
    file: PathBuf,
    /// placement slot name, or several joined with `+`
    slot: String,
    /// integer upscale factor (1, 2 or 4) applied with nearest-neighbour
    #[serde(default = "default_logo_scale")]
    scale: u8,
    /// horizontal alignment within the free area (`left`/`center`/`right`);
    /// absent → the slot's outward default
    #[serde(default)]
    h_align: Option<String>,
    /// vertical alignment within the free area (`top`/`center`/`bottom`);
    /// absent → the slot's outward default
    #[serde(default)]
    v_align: Option<String>,
}

/// A multi-line text label to draw onto the map, supplied as one JSON object
/// per `--label` flag. `slot` follows the same single-or-`+`-combined rule as
/// [`LogoSpec`].
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LabelSpec {
    /// placement slot name, or several joined with `+`
    slot: String,
    /// the text, one entry per line
    lines: Vec<String>,
    /// optional path to a TrueType font for this label; absent → the global
    /// `--font`
    #[serde(default)]
    font: Option<PathBuf>,
    /// font size in pixels (must be a positive, finite number)
    font_px: f32,
    /// text colour as a hex string (default `#ffffff`)
    #[serde(default = "default_label_color")]
    color: String,
    /// horizontal alignment within the free area (`left`/`center`/`right`);
    /// absent → the slot's outward default
    #[serde(default)]
    h_align: Option<String>,
    /// vertical alignment within the free area (`top`/`center`/`bottom`);
    /// absent → the slot's outward default
    #[serde(default)]
    v_align: Option<String>,
}

/// Parse a `--logo` JSON object into a [`LogoSpec`].
///
/// # Errors
///
/// fails if the string is not a valid logo JSON object
fn parse_logo_spec(s: &str) -> Result<LogoSpec, serde_json::Error> {
    serde_json::from_str(s)
}

/// Parse a `--label` JSON object into a [`LabelSpec`].
///
/// # Errors
///
/// fails if the string is not a valid label JSON object
fn parse_label_spec(s: &str) -> Result<LabelSpec, serde_json::Error> {
    serde_json::from_str(s)
}

/// Parse a single slot name, or several joined with `+`, into the list of
/// placement slots they reserve. The combined slots are de-duplicated and must
/// form a solid axis-aligned rectangle (see
/// [`PlacementSlot::slots_form_rectangle`](sl_map_apis::coverage::PlacementSlot::slots_form_rectangle)).
/// The first named slot is the group's anchor (it determines the default
/// alignment), and is returned first.
fn parse_slot_group(slot: &str) -> Result<Vec<sl_map_apis::coverage::PlacementSlot>, crate::Error> {
    use sl_map_apis::coverage::PlacementSlot;
    let mut group: Vec<PlacementSlot> = Vec::new();
    for name in slot.split('+') {
        let name = name.trim();
        let parsed: PlacementSlot =
            name.parse()
                .map_err(|err: sl_map_apis::coverage::ParsePlacementSlotError| {
                    crate::Error::Placement(err.to_string())
                })?;
        if !group.contains(&parsed) {
            group.push(parsed);
        }
    }
    if group.is_empty() {
        return Err(crate::Error::Placement(
            "no placement slot supplied".to_owned(),
        ));
    }
    if !PlacementSlot::slots_form_rectangle(&group) {
        return Err(crate::Error::Placement(format!(
            "the combined slot group `{slot}` does not form a solid rectangle"
        )));
    }
    Ok(group)
}

/// Parse an optional horizontal-alignment name; absent/empty → `None` (use the
/// slot's outward default).
fn parse_h_align(
    value: Option<&str>,
) -> Result<Option<sl_map_apis::coverage::HAlign>, crate::Error> {
    use sl_map_apis::coverage::HAlign;
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some("left") => Ok(Some(HAlign::Left)),
        Some("center") => Ok(Some(HAlign::Center)),
        Some("right") => Ok(Some(HAlign::Right)),
        Some(other) => Err(crate::Error::Placement(format!(
            "invalid h_align `{other}`"
        ))),
    }
}

/// Parse an optional vertical-alignment name; absent/empty → `None` (use the
/// slot's outward default).
fn parse_v_align(
    value: Option<&str>,
) -> Result<Option<sl_map_apis::coverage::VAlign>, crate::Error> {
    use sl_map_apis::coverage::VAlign;
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some("top") => Ok(Some(VAlign::Top)),
        Some("center") => Ok(Some(VAlign::Center)),
        Some("bottom") => Ok(Some(VAlign::Bottom)),
        Some(other) => Err(crate::Error::Placement(format!(
            "invalid v_align `{other}`"
        ))),
    }
}

/// Map a legend-slot name to a placement slot (or `None` to hide it). Absent /
/// empty → `TopLeft` (the default); `"none"` → hidden; any of the nine slot
/// names → that slot; anything else is an error.
fn legend_position_from_slot(
    slot: Option<&str>,
) -> Result<Option<sl_map_apis::coverage::PlacementSlot>, crate::Error> {
    use sl_map_apis::coverage::PlacementSlot;
    let name = match slot {
        None => return Ok(Some(PlacementSlot::TopLeft)),
        Some(s) => s.trim(),
    };
    match name {
        "" => Ok(Some(PlacementSlot::TopLeft)),
        "none" => Ok(None),
        other => other
            .parse::<PlacementSlot>()
            .map(Some)
            .map_err(|err| crate::Error::Placement(err.to_string())),
    }
}

/// Shared optional flags for the per-region annotation overlays, the output
/// format, and logo / text-label placement. Flattened into both render
/// subcommands via `#[clap(flatten)]`.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct PlacementArgs {
    /// draw a hairline rectangle around each region
    #[clap(long)]
    pub region_rectangles: bool,
    /// draw each region's name in its lower-left corner (needs a font, from
    /// --region-font or the global --font)
    #[clap(long)]
    pub region_names: bool,
    /// draw each region's (x, y) grid coordinates (needs a font)
    #[clap(long)]
    pub region_coordinates: bool,
    /// TrueType font for the region name/coordinate overlay; defaults to the
    /// global --font
    #[clap(long)]
    pub region_font: Option<PathBuf>,
    /// output image format (png or jpeg)
    #[clap(long, value_enum, default_value_t = OutputFormat::Png)]
    pub format: OutputFormat,
    /// a logo image to place, as a JSON object (repeatable), e.g.
    /// `--logo '{"file":"logo.png","slot":"bottom_right","scale":2}'`. The
    /// `slot` may join several names with `+` to combine them into one larger
    /// rectangle (which must form a solid rectangle).
    #[clap(long = "logo", value_parser = parse_logo_spec)]
    pub logos: Vec<LogoSpec>,
    /// a multi-line text label to draw, as a JSON object (repeatable), e.g.
    /// `--label '{"slot":"top_right","font_px":24,"lines":["RRMC","2026-04-18"]}'`.
    #[clap(long = "label", value_parser = parse_label_spec)]
    pub labels: Vec<LabelSpec>,
}

impl PlacementArgs {
    /// the region-overlay options selected by the flags
    const fn region_overlay(&self) -> RegionOverlayOptions {
        RegionOverlayOptions {
            rectangles: self.region_rectangles,
            names: self.region_names,
            coordinates: self.region_coordinates,
        }
    }

    /// whether any logo or text label was requested (i.e. whether placement
    /// planning needs to run)
    const fn has_placements(&self) -> bool {
        !self.logos.is_empty() || !self.labels.is_empty()
    }
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
    /// hex colour for the per-shape override labels.
    #[clap(long, value_parser = parse_color)]
    pub glw_label_color: Option<image::Rgba<u8>>,
    /// placement slot for the base legend: one of the nine slot names
    /// (`top_left` … `bottom_right`), or `none` to hide the legend. Defaults
    /// to `top_left`.
    #[clap(long)]
    pub glw_legend_slot: Option<String>,
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

    /// Build a [`sl_glw::GlwStyle`] from the library default, placing the
    /// legend in the requested slot (defaulting to `TopLeft`, or hidden when
    /// `--glw-legend-slot none`), then apply any CLI colour overrides the user
    /// supplied.
    ///
    /// # Errors
    ///
    /// returns [`Error::Placement`] if `--glw-legend-slot` is not a valid slot
    /// name (or `none`)
    fn build_style(&self) -> Result<sl_glw::GlwStyle, crate::Error> {
        let mut style = sl_glw::GlwStyle {
            legend_position: legend_position_from_slot(self.glw_legend_slot.as_deref())?,
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
        if let Some(c) = self.glw_label_color {
            style.palette.label_fg = c;
        }
        Ok(style)
    }

    /// The placement slot the base legend occupies for these args, or `None`
    /// when no GLW overlay is requested, the legend is hidden, or the slot is
    /// invalid (the invalid case surfaces as an error from [`Self::build_style`]
    /// when the overlay is actually drawn).
    fn legend_slot(&self) -> Option<sl_map_apis::coverage::PlacementSlot> {
        self.source()?;
        legend_position_from_slot(self.glw_legend_slot.as_deref())
            .ok()
            .flatten()
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
    /// optional region-overlay, output-format and logo/label placement flags
    #[clap(flatten)]
    pub placement: PlacementArgs,
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
    /// Report which placement slots are free for a given render (no image is
    /// produced); useful for deciding where to put a logo or label
    PlacementSlots(PlacementSlots),
    /// Measure the pixel size a text label would render at, to help pick a font
    /// size or check whether it fits a slot (no image is produced)
    MeasureText(MeasureText),
}

/// Report the free placement slots for a render described by a grid rectangle
/// or a USB notecard route. The occupancy is measured on a blank overlay-only
/// map (route + optional GLW shapes), so no map tiles are fetched.
#[derive(clap::Parser, Debug)]
pub struct PlacementSlots {
    /// the x coordinate of the lower left corner of the grid rectangle (use
    /// the four corner flags, or --usb-notecard, to choose the area)
    #[clap(long, requires_all = ["lower_left_y", "upper_right_x", "upper_right_y"],
           conflicts_with = "usb_notecard")]
    pub lower_left_x: Option<u32>,
    /// the y coordinate of the lower left corner of the grid rectangle
    #[clap(long)]
    pub lower_left_y: Option<u32>,
    /// the x coordinate of the upper right corner of the grid rectangle
    #[clap(long)]
    pub upper_right_x: Option<u32>,
    /// the y coordinate of the upper right corner of the grid rectangle
    #[clap(long)]
    pub upper_right_y: Option<u32>,
    /// a USB notecard whose route defines the area (and is drawn onto the
    /// occupancy map). Mutually exclusive with the grid-rectangle corners.
    #[clap(long)]
    pub usb_notecard: Option<PathBuf>,
    /// the colour to use for the route on the occupancy map
    #[clap(long, value_parser = parse_color, default_value = "#f00")]
    pub color: image::Rgba<u8>,
    /// the maximum width of the (virtual) output image in pixels
    #[clap(long)]
    pub max_width: u32,
    /// the maximum height of the (virtual) output image in pixels
    #[clap(long)]
    pub max_height: u32,
    /// a combined slot group to also report, given as slot names joined with
    /// `+` (repeatable), e.g. `--group bottom_left+bottom_center`. The slots
    /// must form a solid rectangle.
    #[clap(long = "group")]
    pub groups: Vec<String>,
    /// optional GLW overlay flags; its shapes count as occupied
    #[clap(flatten)]
    pub glw: GlwOverlayArgs,
}

/// Measure the rendered pixel size of a multi-line text label.
#[derive(clap::Parser, Debug)]
pub struct MeasureText {
    /// TrueType font to measure with; defaults to the global --font
    #[clap(long)]
    pub font: Option<PathBuf>,
    /// font size in pixels
    #[clap(long)]
    pub font_px: f32,
    /// a line of text (repeatable, in order); at least one is required
    #[clap(long = "line", required = true)]
    pub lines: Vec<String>,
}

/// The Clap type for all the commandline parameters
#[derive(clap::Parser, Debug)]
#[clap(name = clap::crate_name!(),
       about = clap::crate_description!(),
       author = clap::crate_authors!(),
       version = clap::crate_version!(),
       )]
struct Options {
    /// cache dir for map tiles and region-name lookups. Required for every
    /// subcommand except `measure-text` (which renders nothing and reads no
    /// cache).
    #[clap(long)]
    cache_dir: Option<PathBuf>,
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
fn output_metadata(
    grid_rectangle: &GridRectangle,
    metadata_output_file: Option<&PathBuf>,
) -> Result<(), crate::Error> {
    let pps_config = format!("PPS HUD config: {}", grid_rectangle.pps_hud_config());
    let aspect = format!(
        "The aspect ratio of the image is {}:{} ({})",
        grid_rectangle.size_x(),
        grid_rectangle.size_y(),
        f64::from(grid_rectangle.size_x()) / f64::from(grid_rectangle.size_y())
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
/// Resolve the GLW event these args request, via the three-tier HTTP cache
/// rooted at `cache_dir` (for `--glw-event-id` / `--glw-event-key`) or by
/// reading a JSON file (for `--glw-input-file`). Returns `Ok(None)` when no
/// GLW source was requested or the server returned no event for the requested
/// id/key. Performs no file writes and no drawing, so it is safe to call from
/// both [`fetch_and_draw_glw`] and the placement-occupancy builder.
async fn resolve_glw_event(
    cache_dir: &std::path::Path,
    args: &GlwOverlayArgs,
) -> Result<Option<sl_glw::GlwEvent>, crate::Error> {
    let Some(source) = args.source() else {
        return Ok(None);
    };
    let event = match source {
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
    Ok(event)
}

/// Draw the GLW overlay these args request onto `map`: resolve the event (via
/// [`resolve_glw_event`]), load the supplied font, optionally write the event
/// back out to `--glw-output-file`, and draw it with the user's style. Returns
/// `Ok(true)` when an overlay was drawn, `Ok(false)` when no source was
/// requested or no event was found. Returns [`Error::FontRequired`] if a source
/// was requested but `font_path` is `None`.
async fn fetch_and_draw_glw(
    cache_dir: &std::path::Path,
    font_path: Option<&std::path::Path>,
    args: &GlwOverlayArgs,
    map: &mut Map,
) -> Result<bool, crate::Error> {
    use sl_glw::MapLikeGlwExt as _;
    if args.source().is_none() {
        return Ok(false);
    }
    let Some(font_path) = font_path else {
        return Err(crate::Error::FontRequired);
    };
    let font = sl_map_apis::text::load_font(font_path)?;

    let Some(event) = resolve_glw_event(cache_dir, args).await? else {
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

    let style = args.build_style()?;
    map.draw_glw_event_with_font(&event, &style, &font);
    Ok(true)
}

// =====================================================================
// Per-region annotation overlay — rectangles, names, grid coordinates.
// Ported from sl-map-web's render route so the CLI and web agree.
// =====================================================================

/// Minimum rendered size of a region, in pixels, for the per-region name and
/// grid-coordinate text overlays to be drawn. Below this the text does not fit
/// a region and would smear across its neighbours, so it is skipped (the cheap
/// rectangle outline is still drawn). 64 px corresponds to zoom level 3 or
/// lower.
const MIN_PIXELS_PER_REGION_FOR_REGION_LABELS: f32 = 64.0;

/// Maximum number of regions in a render for which the per-region name and
/// grid-coordinate overlays are drawn. Each region name is an individual
/// upstream lookup (cached, but cold on first use), so a huge rectangle would
/// fan out into thousands of requests.
const MAX_REGIONS_FOR_REGION_LABELS: usize = 1024;

/// Fraction of a region's rendered pixel size used as the region-label font
/// size, before clamping. Keeps the text proportional to the zoom.
const REGION_LABEL_FONT_FACTOR: f32 = 0.12;

/// Lower clamp for the region-label font size in pixels.
const REGION_LABEL_FONT_MIN_PX: f32 = 8.0;

/// Upper clamp for the region-label font size in pixels.
const REGION_LABEL_FONT_MAX_PX: f32 = 22.0;

/// Padding in pixels between a region's lower-left corner and the text block
/// drawn inside it.
const REGION_LABEL_PADDING: i32 = 3;

/// Colour of the per-region rectangle outline (opaque white).
const REGION_RECTANGLE_COLOR: image::Rgba<u8> = image::Rgba([255, 255, 255, 255]);

/// Which of the optional per-region annotation overlays to draw. All three are
/// independent and may be combined.
#[derive(Debug, Clone, Copy, Default)]
struct RegionOverlayOptions {
    /// draw a hairline rectangle around each region.
    rectangles: bool,
    /// draw each region's name in its lower-left corner.
    names: bool,
    /// draw each region's `(x, y)` grid coordinates (above the name when both
    /// are enabled).
    coordinates: bool,
}

impl RegionOverlayOptions {
    /// whether any overlay at all is requested.
    const fn any(self) -> bool {
        self.rectangles || self.names || self.coordinates
    }

    /// whether any text overlay (name or coordinates) is requested. Text is the
    /// part gated by region size and count.
    const fn any_text(self) -> bool {
        self.names || self.coordinates
    }
}

/// Whether the per-region name / coordinate text overlay should be drawn for a
/// render whose regions render at `pixels_per_region` pixels each and which
/// covers `region_count` regions. Both gates must pass.
const fn region_text_overlay_allowed(pixels_per_region: f32, region_count: usize) -> bool {
    pixels_per_region >= MIN_PIXELS_PER_REGION_FOR_REGION_LABELS
        && region_count <= MAX_REGIONS_FOR_REGION_LABELS
}

/// The pixel rectangle `(left, top, width, height)` a region occupies in `map`,
/// or `None` if the region is outside it.
fn region_pixel_rect(map: &Map, grid: &GridCoordinates) -> Option<(u32, u32, u32, u32)> {
    use sl_map_apis::map_tiles::MapLike as _;
    let (x0, y0) = map.pixel_coordinates_for_coordinates(
        grid,
        &sl_types::map::RegionCoordinates::new(0f32, 0f32, 0f32),
    )?;
    let (x1, y1) = map.pixel_coordinates_for_coordinates(
        grid,
        &sl_types::map::RegionCoordinates::new(256f32, 256f32, 0f32),
    )?;
    Some((x0.min(x1), y0.min(y1), x0.abs_diff(x1), y0.abs_diff(y1)))
}

/// Draw a hairline outline around every region of `map`.
fn draw_region_rectangles(map: &mut Map) {
    use sl_map_apis::map_tiles::MapLike as _;
    use sl_types::map::GridRectangleLike as _;
    for x in map.x_range() {
        for y in map.y_range() {
            let grid = GridCoordinates::new(x, y);
            if let Some((left, top, width, height)) = region_pixel_rect(map, &grid) {
                map.draw_hollow_rect(left, top, width, height, REGION_RECTANGLE_COLOR);
            }
        }
    }
}

/// Draw the requested per-region annotations onto `map`. The rectangle outline
/// is always drawn when requested; the name / coordinate text is gated by
/// region size and count (see [`region_text_overlay_allowed`]) and needs a
/// font. `font_path` is the region-specific font, already resolved to fall
/// back to the global `--font`.
async fn apply_region_overlay(
    opts: RegionOverlayOptions,
    font_path: Option<&std::path::Path>,
    region_cache: &mut RegionNameToGridCoordinatesCache,
    map: &mut Map,
) -> Result<(), crate::Error> {
    use sl_map_apis::map_tiles::MapLike as _;
    use sl_types::map::GridRectangleLike as _;
    if !opts.any() {
        return Ok(());
    }
    let pixels_per_region = map.pixels_per_region();
    let region_count =
        usize::try_from(u64::from(map.size_x()).saturating_mul(u64::from(map.size_y())))
            .unwrap_or(usize::MAX);
    let run_loop = opts.any_text() && region_text_overlay_allowed(pixels_per_region, region_count);
    if !run_loop {
        if opts.any_text() {
            tracing::info!(
                pixels_per_region,
                region_count,
                "skipping region name/coordinate overlay (regions too small or too many)"
            );
        }
        if opts.rectangles {
            draw_region_rectangles(map);
        }
        return Ok(());
    }
    let font_path = font_path.ok_or(crate::Error::TextFontRequired(
        "region name/coordinate overlay",
    ))?;
    let font = sl_map_apis::text::load_font(font_path)?;
    let scale = ab_glyph::PxScale::from(
        (pixels_per_region * REGION_LABEL_FONT_FACTOR)
            .clamp(REGION_LABEL_FONT_MIN_PX, REGION_LABEL_FONT_MAX_PX),
    );
    let style = sl_map_apis::text::LabelStyle {
        scale,
        fg: image::Rgba([255, 255, 255, 255]),
        shadow: image::Rgba([0, 0, 0, 180]),
        align: sl_map_apis::coverage::HAlign::Left,
    };
    for x in map.x_range() {
        for y in map.y_range() {
            let grid = GridCoordinates::new(x, y);
            let Some((left, top, width, height)) = region_pixel_rect(map, &grid) else {
                continue;
            };
            let mut region_name: Option<String> = None;
            if opts.names {
                match region_cache.get_region_name(&grid).await {
                    Ok(Some(name)) => region_name = Some(name.to_string()),
                    Ok(None) => {}
                    Err(err) => {
                        tracing::debug!("region name lookup failed for {grid:?}: {err}");
                    }
                }
            }
            if opts.rectangles {
                map.draw_hollow_rect(left, top, width, height, REGION_RECTANGLE_COLOR);
            }
            let mut lines: Vec<String> = Vec::new();
            if opts.coordinates {
                lines.push(format!("({x}, {y})"));
            }
            if let Some(name) = region_name {
                lines.push(name);
            }
            if lines.is_empty() {
                continue;
            }
            let (_text_w, text_h) = sl_map_apis::text::measure_text(scale, &font, &lines);
            // SL y points up while image y points down, so the region's bottom
            // edge (the text anchor) is `top + height`.
            let bottom = top.saturating_add(height);
            let origin_x = i32::try_from(left)
                .unwrap_or(0)
                .saturating_add(REGION_LABEL_PADDING);
            let origin_y = i32::try_from(bottom)
                .unwrap_or(0)
                .saturating_sub(REGION_LABEL_PADDING)
                .saturating_sub(i32::try_from(text_h).unwrap_or(0));
            map.draw_text_label((origin_x, origin_y), &lines, &style, &font);
        }
    }
    Ok(())
}

// =====================================================================
// Logo / text-label placement — planning and execution.
// Ported from sl-map-web's render route, using file paths instead of
// the web's font_id / DB-backed logo storage.
// =====================================================================

/// One accepted label, ready to draw by [`execute_labels`].
struct LabelDraw {
    /// the text, one entry per line.
    lines: Vec<String>,
    /// the resolved font to render with.
    font: ab_glyph::FontVec,
    /// colour, scale and shadow for the text.
    style: sl_map_apis::text::LabelStyle,
    /// top-left pixel origin within the image.
    origin: (i32, i32),
}

/// One accepted logo (already scaled), ready to composite by [`execute_logos`].
struct LogoDraw {
    /// the decoded (and optionally scaled) logo bitmap.
    img: image::RgbaImage,
    /// x pixel coordinate of the logo's top-left corner.
    x: i64,
    /// y pixel coordinate of the logo's top-left corner.
    y: i64,
}

/// Resolve a placement to the slots it reserves and the pixel rectangle to fit
/// content into. A single-slot placement uses that slot's own free rectangle; a
/// combined placement uses the largest free rectangle within exactly those
/// slots' thirds. `group`'s first element is the anchor.
fn resolve_placement(
    group: &[sl_map_apis::coverage::PlacementSlot],
    slots: &[sl_map_apis::coverage::PlacementSlotInfo],
    grid: &sl_map_apis::coverage::OccupancyGrid,
) -> Result<
    (
        Vec<sl_map_apis::coverage::PlacementSlot>,
        sl_map_apis::coverage::PixelRect,
    ),
    crate::Error,
> {
    let anchor = *group
        .first()
        .ok_or_else(|| crate::Error::Placement("empty slot group".to_owned()))?;
    if group.len() > 1 {
        let rect = grid.subset_rect(group).ok_or_else(|| {
            crate::Error::Placement(format!(
                "the combined slot at `{anchor}` is fully covered; no room for content"
            ))
        })?;
        Ok((group.to_vec(), rect))
    } else {
        let info = slots
            .iter()
            .find(|info| info.slot == anchor)
            .ok_or_else(|| crate::Error::Placement(format!("slot `{anchor}` not found")))?;
        let rect = info.free_rect.ok_or_else(|| {
            crate::Error::Placement(format!(
                "slot `{anchor}` is fully covered; no room for content"
            ))
        })?;
        Ok((vec![anchor], rect))
    }
}

/// Reserve a placement's slots in the shared pool, rejecting any clash with the
/// legend, with already-reserved slots from the *other* placement kind
/// (`others`), or with slots reserved earlier in this pass (`used`). `what`
/// names the placement kind for the error message.
fn reserve(
    reserved: &[sl_map_apis::coverage::PlacementSlot],
    used: &mut Vec<sl_map_apis::coverage::PlacementSlot>,
    others: &[sl_map_apis::coverage::PlacementSlot],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    what: &str,
) -> Result<(), crate::Error> {
    for &slot in reserved {
        if legend_slot == Some(slot) {
            return Err(crate::Error::Placement(format!(
                "a {what} uses slot `{slot}` which is occupied by the legend"
            )));
        }
        if used.contains(&slot) || others.contains(&slot) {
            return Err(crate::Error::Placement(format!(
                "two placements target the same slot `{slot}`"
            )));
        }
        used.push(slot);
    }
    Ok(())
}

/// Plan the text labels against the free space measured on `occupancy` (an
/// overlay-only map carrying just the route + GLW shapes). Rejects any label
/// that overflows its free space or clashes with the legend, another label, or
/// a slot already reserved by a logo (`reserved_by_others`). Returns the draw
/// list and the slots the labels reserved.
fn plan_labels(
    global_font: Option<&std::path::Path>,
    labels: &[LabelSpec],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    reserved_by_others: &[sl_map_apis::coverage::PlacementSlot],
    occupancy: &Map,
) -> Result<(Vec<LabelDraw>, Vec<sl_map_apis::coverage::PlacementSlot>), crate::Error> {
    let mut used: Vec<sl_map_apis::coverage::PlacementSlot> = Vec::new();
    let mut draws: Vec<LabelDraw> = Vec::new();
    if labels.is_empty() {
        return Ok((draws, used));
    }
    let grid = sl_map_apis::coverage::OccupancyGrid::from_map(
        occupancy,
        sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
    );
    let slots = grid.evaluate_slots();
    for label in labels {
        let lines: Vec<String> = label.lines.clone();
        if lines.iter().all(|line| line.trim().is_empty()) {
            continue;
        }
        if !(label.font_px.is_finite() && label.font_px > 0f32) {
            return Err(crate::Error::Placement(format!(
                "label font size must be a positive number of pixels, got {}",
                label.font_px
            )));
        }
        let group = parse_slot_group(&label.slot)?;
        let anchor = *group
            .first()
            .ok_or_else(|| crate::Error::Placement("empty slot group".to_owned()))?;
        let (reserved, rect) = resolve_placement(&group, &slots, &grid)?;
        reserve(
            &reserved,
            &mut used,
            reserved_by_others,
            legend_slot,
            "label",
        )?;
        let font_path = label
            .font
            .as_deref()
            .or(global_font)
            .ok_or(crate::Error::TextFontRequired("a text label"))?;
        let font = sl_map_apis::text::load_font(font_path)?;
        let color = parse_color(label.color.trim()).map_err(|err| {
            crate::Error::Placement(format!("invalid label color `{}`: {err}", label.color))
        })?;
        let scale = ab_glyph::PxScale::from(label.font_px);
        let (text_w, text_h) = sl_map_apis::text::measure_text(scale, &font, &lines);
        if text_w > rect.width || text_h > rect.height {
            return Err(crate::Error::Placement(format!(
                "label text renders at {text_w}x{text_h} px but the free area at slot `{anchor}` only has {}x{} px",
                rect.width, rect.height
            )));
        }
        let (default_h, default_v) = anchor.default_alignment();
        let h = parse_h_align(label.h_align.as_deref())?.unwrap_or(default_h);
        let v = parse_v_align(label.v_align.as_deref())?.unwrap_or(default_v);
        let origin_x = rect.x.saturating_add(h.offset(text_w, rect.width));
        let origin_y = rect.y.saturating_add(v.offset(text_h, rect.height));
        draws.push(LabelDraw {
            lines,
            font,
            style: sl_map_apis::text::LabelStyle {
                scale,
                fg: color,
                shadow: image::Rgba([0, 0, 0, 180]),
                align: h,
            },
            origin: (
                i32::try_from(origin_x).unwrap_or(0),
                i32::try_from(origin_y).unwrap_or(0),
            ),
        });
    }
    Ok((draws, used))
}

/// Draw a planned label list onto `map` (above the route and GLW overlay).
fn execute_labels(draws: &[LabelDraw], map: &mut Map) {
    use sl_map_apis::map_tiles::MapLike as _;
    for d in draws {
        map.draw_text_label(d.origin, &d.lines, &d.style, &d.font);
    }
}

/// Plan the logo placements against the free space measured on `occupancy`.
/// Each logo is loaded from its file, optionally integer-scaled
/// (nearest-neighbour), and aligned within its free rectangle. Rejects any logo
/// that overflows, has an invalid scale, or clashes with the legend, another
/// logo, or a slot already reserved by a label (`reserved_by_others`).
fn plan_logos(
    logos: &[LogoSpec],
    legend_slot: Option<sl_map_apis::coverage::PlacementSlot>,
    reserved_by_others: &[sl_map_apis::coverage::PlacementSlot],
    occupancy: &Map,
) -> Result<(Vec<LogoDraw>, Vec<sl_map_apis::coverage::PlacementSlot>), crate::Error> {
    let mut used: Vec<sl_map_apis::coverage::PlacementSlot> = Vec::new();
    let mut draws: Vec<LogoDraw> = Vec::new();
    if logos.is_empty() {
        return Ok((draws, used));
    }
    let grid = sl_map_apis::coverage::OccupancyGrid::from_map(
        occupancy,
        sl_map_apis::coverage::DEFAULT_COVERAGE_GRID,
    );
    let slots = grid.evaluate_slots();
    for logo in logos {
        if logo.scale != 1 && logo.scale != 2 && logo.scale != 4 {
            return Err(crate::Error::Placement(format!(
                "logo scale must be 1, 2 or 4, got {}",
                logo.scale
            )));
        }
        let group = parse_slot_group(&logo.slot)?;
        let anchor = *group
            .first()
            .ok_or_else(|| crate::Error::Placement("empty slot group".to_owned()))?;
        let (reserved, rect) = resolve_placement(&group, &slots, &grid)?;
        reserve(
            &reserved,
            &mut used,
            reserved_by_others,
            legend_slot,
            "logo",
        )?;
        let decoded = image::open(&logo.file)?;
        let mut rgba = decoded.to_rgba8();
        if logo.scale != 1 {
            let factor = u32::from(logo.scale);
            let (w, h) = (rgba.width(), rgba.height());
            rgba = image::imageops::resize(
                &rgba,
                w.saturating_mul(factor),
                h.saturating_mul(factor),
                image::imageops::FilterType::Nearest,
            );
        }
        let (w, h) = (rgba.width(), rgba.height());
        if w > rect.width || h > rect.height {
            return Err(crate::Error::Placement(format!(
                "logo renders at {w}x{h} px but the free area at slot `{anchor}` only has {}x{} px",
                rect.width, rect.height
            )));
        }
        let (default_h, default_v) = anchor.default_alignment();
        let hh = parse_h_align(logo.h_align.as_deref())?.unwrap_or(default_h);
        let vv = parse_v_align(logo.v_align.as_deref())?.unwrap_or(default_v);
        let origin_x = rect.x.saturating_add(hh.offset(w, rect.width));
        let origin_y = rect.y.saturating_add(vv.offset(h, rect.height));
        draws.push(LogoDraw {
            img: rgba,
            x: i64::from(origin_x),
            y: i64::from(origin_y),
        });
    }
    Ok((draws, used))
}

/// Composite a planned logo list onto `map` with alpha blending (above the
/// route, GLW overlay and labels).
fn execute_logos(draws: &[LogoDraw], map: &mut Map) {
    use sl_map_apis::map_tiles::MapLike as _;
    for d in draws {
        image::imageops::overlay(map.image_mut(), &d.img, d.x, d.y);
    }
}

/// Build the overlay-only occupancy map (a blank base sized to the final image,
/// plus the GLW shapes and — for notecard renders — the route) and plan the
/// labels and logos against it. The occupancy is measured on a blank base (the
/// opaque map tiles would otherwise count as fully covered), so this needs no
/// map-tile fetch and can run before the real render to reject an over-full
/// placement up front.
#[expect(
    clippy::too_many_arguments,
    reason = "this gathers the full render context (area, dimensions, overlay, route, placements) for a single pre-render pass"
)]
async fn plan_placements(
    cache_dir: &std::path::Path,
    global_font: Option<&std::path::Path>,
    occ_rect: GridRectangle,
    max_width: u32,
    max_height: u32,
    glw: &GlwOverlayArgs,
    route: Option<(
        &mut RegionNameToGridCoordinatesCache,
        &USBNotecard,
        image::Rgba<u8>,
    )>,
    labels: &[LabelSpec],
    logos: &[LogoSpec],
) -> Result<(Vec<LabelDraw>, Vec<LogoDraw>), crate::Error> {
    use sl_glw::MapLikeGlwExt as _;
    let legend_slot = glw.legend_slot();
    let mut occ = Map::blank_fit(occ_rect, max_width, max_height)?;
    if glw.source().is_some() {
        let font_path = global_font.ok_or(crate::Error::FontRequired)?;
        let font = sl_map_apis::text::load_font(font_path)?;
        if let Some(event) = resolve_glw_event(cache_dir, glw).await? {
            // Exclude the legend from the occupancy: it is a placement
            // candidate, not a constraint, so it must not block its own slot.
            let mut style = glw.build_style()?;
            style.legend_position = None;
            occ.draw_glw_event_with_font(&event, &style, &font);
        }
    }
    if let Some((region_cache, notecard, color)) = route {
        occ.draw_route_with_progress(region_cache, notecard, color, None)
            .await?;
    }
    let (label_draws, label_slots) = plan_labels(global_font, labels, legend_slot, &[], &occ)?;
    let (logo_draws, _) = plan_logos(logos, legend_slot, &label_slots, &occ)?;
    Ok((label_draws, logo_draws))
}

/// Maximum width or height (per side) of a rendered image in pixels.
const MAX_OUTPUT_DIMENSION: u32 = 0x8000;

/// Maximum area (`max_width * max_height`) of a rendered image in pixels
/// (16 384² = 0x1000_0000 ≈ 268 M pixels ≈ 1 GiB for an RGBA buffer).
const MAX_OUTPUT_AREA: u64 = 0x1000_0000;

/// Reject output dimensions outside the allowed bounds. Both sides must be
/// greater than zero and at most [`MAX_OUTPUT_DIMENSION`], and their product at
/// most [`MAX_OUTPUT_AREA`].
fn validate_dimensions(max_width: u32, max_height: u32) -> Result<(), crate::Error> {
    if max_width == 0 || max_height == 0 {
        return Err(crate::Error::InvalidDimensions(
            "max_width and max_height must be greater than zero".to_owned(),
        ));
    }
    if max_width > MAX_OUTPUT_DIMENSION || max_height > MAX_OUTPUT_DIMENSION {
        return Err(crate::Error::InvalidDimensions(format!(
            "max_width and max_height must each be <= {MAX_OUTPUT_DIMENSION}"
        )));
    }
    if u64::from(max_width).saturating_mul(u64::from(max_height)) > MAX_OUTPUT_AREA {
        return Err(crate::Error::InvalidDimensions(format!(
            "max_width * max_height must be <= {MAX_OUTPUT_AREA} pixels"
        )));
    }
    Ok(())
}

/// Run the `placement-slots` subcommand: build a blank overlay-only occupancy
/// map (route + optional GLW shapes), evaluate the nine slots and any requested
/// combined groups, and print a human-readable table to stdout.
async fn run_placement_slots(
    cache_dir: &std::path::Path,
    font: Option<&std::path::Path>,
    args: &PlacementSlots,
) -> Result<(), crate::Error> {
    use image::GenericImageView as _;
    use sl_glw::MapLikeGlwExt as _;
    use sl_map_apis::coverage::{DEFAULT_COVERAGE_GRID, OccupancyGrid, PlacementSlot};
    validate_dimensions(args.max_width, args.max_height)?;
    let mut region_cache = RegionNameToGridCoordinatesCache::new(cache_dir.to_owned())?;
    let (grid_rectangle, notecard) = if let Some(path) = &args.usb_notecard {
        let notecard = USBNotecard::load_from_file(path)?;
        let rect = usb_notecard_to_grid_rectangle(&mut region_cache, &notecard).await?;
        (rect, Some(notecard))
    } else if let (Some(llx), Some(lly), Some(urx), Some(ury)) = (
        args.lower_left_x,
        args.lower_left_y,
        args.upper_right_x,
        args.upper_right_y,
    ) {
        (
            GridRectangle::new(
                GridCoordinates::new(llx, lly),
                GridCoordinates::new(urx, ury),
            ),
            None,
        )
    } else {
        return Err(crate::Error::Placement(
            "provide either --usb-notecard or all four grid-rectangle corner flags".to_owned(),
        ));
    };
    let mut occ = Map::blank_fit(grid_rectangle, args.max_width, args.max_height)?;
    if args.glw.source().is_some() {
        let font_path = font.ok_or(crate::Error::FontRequired)?;
        let glw_font = sl_map_apis::text::load_font(font_path)?;
        if let Some(event) = resolve_glw_event(cache_dir, &args.glw).await? {
            let mut style = args.glw.build_style()?;
            style.legend_position = None;
            occ.draw_glw_event_with_font(&event, &style, &glw_font);
        }
    }
    if let Some(notecard) = &notecard {
        occ.draw_route_with_progress(&mut region_cache, notecard, args.color, None)
            .await?;
    }
    // Validate the requested groups (the rectangle rule lives in
    // parse_slot_group) before building the grid.
    let mut groups: Vec<Vec<PlacementSlot>> = Vec::with_capacity(args.groups.len());
    for g in &args.groups {
        groups.push(parse_slot_group(g)?);
    }
    let grid = OccupancyGrid::from_map(&occ, DEFAULT_COVERAGE_GRID);
    let slots = grid.evaluate_slots();
    let (image_width, image_height) = occ.dimensions();
    println!("image: {image_width}x{image_height} px");
    println!(
        "{:<14} {:<9} {:<20} {:<9} connected_neighbours",
        "slot", "available", "free_rect(x,y,w,h)", "occupied"
    );
    for info in &slots {
        let rect = info.free_rect.map_or_else(
            || "-".to_owned(),
            |r| format!("{},{},{},{}", r.x, r.y, r.width, r.height),
        );
        let neighbours = info
            .connected_neighbours
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{:<14} {:<9} {:<20} {:<8.0}% {neighbours}",
            info.slot.as_str(),
            info.available,
            rect,
            info.occupied_fraction * 100.0,
        );
    }
    if !groups.is_empty() {
        println!("groups:");
        for (group, names) in groups.iter().zip(args.groups.iter()) {
            let rect = grid.subset_rect(group).map_or_else(
                || "fully covered".to_owned(),
                |r| format!("{},{},{},{}", r.x, r.y, r.width, r.height),
            );
            println!("  {names}: {rect}");
        }
    }
    Ok(())
}

/// Run the `measure-text` subcommand: load the font and print the pixel size
/// the given lines render at.
fn run_measure_text(
    global_font: Option<&std::path::Path>,
    args: &MeasureText,
) -> Result<(), crate::Error> {
    if !(args.font_px.is_finite() && args.font_px > 0f32) {
        return Err(crate::Error::Placement(format!(
            "font size must be a positive number of pixels, got {}",
            args.font_px
        )));
    }
    let font_path = args
        .font
        .as_deref()
        .or(global_font)
        .ok_or(crate::Error::TextFontRequired("measure-text"))?;
    let font = sl_map_apis::text::load_font(font_path)?;
    let scale = ab_glyph::PxScale::from(args.font_px);
    let (width, height) = sl_map_apis::text::measure_text(scale, &font, &args.lines);
    println!("{width}x{height} px ({} line(s))", args.lines.len());
    Ok(())
}

/// Save `map` to `path` in the chosen format. PNG uses the library's `save`
/// (which infers the encoder from the path extension and matches the existing
/// behaviour); JPEG is encoded explicitly so the format is honoured regardless
/// of the output file's extension.
fn save_map(map: &Map, path: &std::path::Path, format: OutputFormat) -> Result<(), crate::Error> {
    use sl_map_apis::map_tiles::MapLike as _;
    match format {
        OutputFormat::Png => map.save(path)?,
        OutputFormat::Jpeg => {
            let mut file = fs_err::File::create(path)?;
            map.image().write_to(&mut file, format.image_format())?;
        }
    }
    Ok(())
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
            let cache_dir = cache_dir.ok_or(crate::Error::CacheDirRequired)?;
            let grid_rectangle: GridRectangle = (&from_grid_rectangle).into();
            let placement = &from_grid_rectangle.placement;
            // Plan logo/label placements up front, against a blank overlay-only
            // occupancy map (no tile fetch), so an over-full slot or bad spec
            // fails before the expensive tile download.
            let (label_draws, logo_draws) = if placement.has_placements() {
                plan_placements(
                    &cache_dir,
                    font.as_deref(),
                    grid_rectangle.to_owned(),
                    from_grid_rectangle.max_width,
                    from_grid_rectangle.max_height,
                    &from_grid_rectangle.glw,
                    None,
                    &placement.labels,
                    &placement.logos,
                )
                .await?
            } else {
                (Vec::new(), Vec::new())
            };
            let ratelimiter = ratelimit::Ratelimiter::builder(10).build()?;
            let mut map_tile_cache = MapTileCache::new(cache_dir.clone(), Some(ratelimiter));
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
            // Layering: base map → region overlay → GLW overlay → labels →
            // logos → save. No route in this branch.
            let region_opts = placement.region_overlay();
            if region_opts.any() {
                let region_font = placement.region_font.as_deref().or(font.as_deref());
                let mut region_cache = RegionNameToGridCoordinatesCache::new(cache_dir.clone())?;
                apply_region_overlay(region_opts, region_font, &mut region_cache, &mut map).await?;
            }
            fetch_and_draw_glw(
                &cache_dir,
                font.as_deref(),
                &from_grid_rectangle.glw,
                &mut map,
            )
            .await?;
            execute_labels(&label_draws, &mut map);
            execute_logos(&logo_draws, &mut map);
            save_map(&map, &from_grid_rectangle.output_file, placement.format)?;
            output_metadata(
                &grid_rectangle,
                from_grid_rectangle.metadata_output_file.as_ref(),
            )?;
        }
        Command::FromUSBNotecard(from_usb_notecard) => {
            let cache_dir = cache_dir.ok_or(crate::Error::CacheDirRequired)?;
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
            let placement = &from_usb_notecard.placement;
            // Plan placements up front, measuring occupancy on a blank map that
            // carries the route (and GLW shapes), so a clash fails before the
            // tile fetch.
            let (label_draws, logo_draws) = if placement.has_placements() {
                plan_placements(
                    &cache_dir,
                    font.as_deref(),
                    grid_rectangle.to_owned(),
                    from_usb_notecard.max_width,
                    from_usb_notecard.max_height,
                    &from_usb_notecard.glw,
                    Some((
                        &mut region_name_to_grid_coordinates_cache,
                        &usb_notecard,
                        from_usb_notecard.color,
                    )),
                    &placement.labels,
                    &placement.logos,
                )
                .await?
            } else {
                (Vec::new(), Vec::new())
            };
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
            // Optional no-overlay-no-route diagnostic save happens BEFORE any
            // overlays are drawn so the file reflects only the base map.
            if let Some(output_file_without_route) = &from_usb_notecard.output_file_without_route {
                save_map(&map, output_file_without_route, placement.format)?;
            }
            // Layering: base map → region overlay → GLW overlay → route →
            // labels → logos → save. The route stays the most-readable element
            // under the labels/logos.
            let region_opts = placement.region_overlay();
            if region_opts.any() {
                let region_font = placement.region_font.as_deref().or(font.as_deref());
                apply_region_overlay(
                    region_opts,
                    region_font,
                    &mut region_name_to_grid_coordinates_cache,
                    &mut map,
                )
                .await?;
            }
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
            execute_labels(&label_draws, &mut map);
            execute_logos(&logo_draws, &mut map);
            save_map(&map, &from_usb_notecard.output_file, placement.format)?;
            output_metadata(
                &grid_rectangle,
                from_usb_notecard.metadata_output_file.as_ref(),
            )?;
        }
        Command::PlacementSlots(placement_slots) => {
            let cache_dir = cache_dir.ok_or(crate::Error::CacheDirRequired)?;
            run_placement_slots(&cache_dir, font.as_deref(), &placement_slots).await?;
        }
        Command::MeasureText(measure_text) => {
            run_measure_text(font.as_deref(), &measure_text)?;
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
    use super::*;
    use pretty_assertions::{assert_eq, assert_matches};

    /// the checked-in DejaVuSans font, relative to this crate's manifest dir
    const FONT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../DejaVuSans.ttf");
    /// the checked-in 128x128 sailing logo, relative to this crate's manifest dir
    const LOGO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sl_sailing_logo.png");

    /// a transparent (fully free) blank map sized so each slot third is large
    /// enough to hold the 128x128 logo at scale 1 but not at scale 4
    fn blank_map() -> Result<Map, Box<dyn std::error::Error>> {
        let rect = GridRectangle::new(
            GridCoordinates::new(1000, 1000),
            GridCoordinates::new(1003, 1003),
        );
        Ok(Map::blank_fit(rect, 1024, 1024)?)
    }

    #[test]
    fn output_format_default_is_png() {
        assert_eq!(OutputFormat::default(), OutputFormat::Png);
    }

    #[test]
    fn parse_logo_spec_parses_and_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let logo = parse_logo_spec(r#"{"file":"logo.png","slot":"top_left"}"#)?;
        assert_eq!(logo.file, PathBuf::from("logo.png"));
        assert_eq!(logo.slot, "top_left");
        assert_eq!(logo.scale, 1, "scale defaults to 1");
        assert_eq!(logo.h_align, None);
        // an unknown field is rejected, and a missing required field too
        assert_matches!(
            parse_logo_spec(r#"{"file":"x.png","slot":"a","bogus":1}"#),
            Err(_)
        );
        assert_matches!(parse_logo_spec(r#"{"slot":"top_left"}"#), Err(_));
        Ok(())
    }

    #[test]
    fn parse_label_spec_parses_and_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let label = parse_label_spec(r#"{"slot":"top_right","font_px":24,"lines":["a","b"]}"#)?;
        assert_eq!(label.slot, "top_right");
        assert_eq!(label.lines, vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(label.color, "#ffffff", "color defaults to white");
        assert_eq!(label.font, None);
        // font_px is required
        assert_matches!(parse_label_spec(r#"{"slot":"a","lines":[]}"#), Err(_));
        Ok(())
    }

    #[test]
    fn parse_slot_group_single_combined_and_rectangle_rule()
    -> Result<(), Box<dyn std::error::Error>> {
        use sl_map_apis::coverage::PlacementSlot as P;
        assert_eq!(parse_slot_group("bottom_right")?, vec![P::BottomRight]);
        // combined, anchor first, de-duplicated
        assert_eq!(
            parse_slot_group("top_left+top_center+top_left")?,
            vec![P::TopLeft, P::TopCenter]
        );
        // a non-rectangular combination is rejected
        assert_matches!(parse_slot_group("top_left+bottom_right"), Err(_));
        assert_matches!(parse_slot_group("top_left+bottom_center"), Err(_));
        // an unknown slot name is rejected
        assert_matches!(parse_slot_group("nowhere"), Err(_));
        Ok(())
    }

    #[test]
    fn legend_position_from_slot_cases() -> Result<(), Box<dyn std::error::Error>> {
        use sl_map_apis::coverage::PlacementSlot as P;
        assert_eq!(legend_position_from_slot(None)?, Some(P::TopLeft));
        assert_eq!(legend_position_from_slot(Some(""))?, Some(P::TopLeft));
        assert_eq!(legend_position_from_slot(Some("none"))?, None);
        assert_eq!(legend_position_from_slot(Some("center"))?, Some(P::Center));
        assert_matches!(legend_position_from_slot(Some("nonsense")), Err(_));
        Ok(())
    }

    #[test]
    fn reserve_rejects_legend_and_double_use() -> Result<(), Box<dyn std::error::Error>> {
        use sl_map_apis::coverage::PlacementSlot as P;
        // clashing with the legend slot is rejected
        let mut used = Vec::new();
        assert_matches!(
            reserve(&[P::TopLeft], &mut used, &[], Some(P::TopLeft), "label"),
            Err(_)
        );
        // a slot already used by the other placement kind is rejected
        let mut used = Vec::new();
        assert_matches!(
            reserve(&[P::TopLeft], &mut used, &[P::TopLeft], None, "logo"),
            Err(_)
        );
        // a fresh slot is accepted and recorded
        let mut used = Vec::new();
        reserve(&[P::TopRight], &mut used, &[], None, "label")?;
        assert_eq!(used, vec![P::TopRight]);
        Ok(())
    }

    #[test]
    fn plan_labels_places_small_and_rejects_oversize() -> Result<(), Box<dyn std::error::Error>> {
        let map = blank_map()?;
        let small = LabelSpec {
            slot: "top_left".to_owned(),
            lines: vec!["hi".to_owned()],
            font: None,
            font_px: 16.0,
            color: "#ffffff".to_owned(),
            h_align: None,
            v_align: None,
        };
        let font = std::path::Path::new(FONT);
        let (draws, used) = plan_labels(Some(font), &[small], None, &[], &map)?;
        assert_eq!(draws.len(), 1);
        assert_eq!(used, vec![sl_map_apis::coverage::PlacementSlot::TopLeft]);

        // a label far too large for its slot's free area is rejected
        let huge = LabelSpec {
            slot: "top_left".to_owned(),
            lines: vec!["WAY TOO BIG".to_owned()],
            font: None,
            font_px: 4000.0,
            color: "#ffffff".to_owned(),
            h_align: None,
            v_align: None,
        };
        assert!(plan_labels(Some(font), &[huge], None, &[], &map).is_err());

        // a label with no font available is rejected
        let needs_font = LabelSpec {
            slot: "top_left".to_owned(),
            lines: vec!["hi".to_owned()],
            font: None,
            font_px: 16.0,
            color: "#ffffff".to_owned(),
            h_align: None,
            v_align: None,
        };
        assert!(plan_labels(None, &[needs_font], None, &[], &map).is_err());
        Ok(())
    }

    #[test]
    fn plan_logos_places_small_and_rejects_oversize() -> Result<(), Box<dyn std::error::Error>> {
        let map = blank_map()?;
        // the 128x128 logo at scale 1 fits a slot third of a 1024 px map
        let small = LogoSpec {
            file: PathBuf::from(LOGO),
            slot: "bottom_right".to_owned(),
            scale: 1,
            h_align: None,
            v_align: None,
        };
        let (draws, used) = plan_logos(&[small], None, &[], &map)?;
        assert_eq!(draws.len(), 1);
        assert_eq!(
            used,
            vec![sl_map_apis::coverage::PlacementSlot::BottomRight]
        );

        // at scale 4 (512x512) it no longer fits a single slot third
        let big = LogoSpec {
            file: PathBuf::from(LOGO),
            slot: "bottom_right".to_owned(),
            scale: 4,
            h_align: None,
            v_align: None,
        };
        assert!(plan_logos(&[big], None, &[], &map).is_err());

        // an invalid scale is rejected
        let bad_scale = LogoSpec {
            file: PathBuf::from(LOGO),
            slot: "bottom_right".to_owned(),
            scale: 3,
            h_align: None,
            v_align: None,
        };
        assert!(plan_logos(&[bad_scale], None, &[], &map).is_err());
        Ok(())
    }

    #[test]
    fn validate_dimensions_bounds() -> Result<(), Box<dyn std::error::Error>> {
        assert_matches!(validate_dimensions(0, 100), Err(_));
        assert_matches!(validate_dimensions(100, 0), Err(_));
        assert_matches!(validate_dimensions(MAX_OUTPUT_DIMENSION + 1, 1), Err(_));
        // within both the per-side and area bounds
        validate_dimensions(2048, 2048)?;
        Ok(())
    }
}
