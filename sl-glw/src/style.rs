//! Visual style for GLW overlays.
//!
//! Defaults were calibrated against the reference image at
//! `example-map-glw.png` (Lalia's "TYC Cruise" map): green outlines on
//! areas/circles, no interior fill, white wind/current arrows, white
//! tilde-style wave glyphs, and white text labels with a 1-pixel black
//! drop shadow for legibility on varied tile backgrounds.

/// Per-element RGBA colours used when drawing a GLW overlay.
#[derive(Debug, Clone, Copy)]
pub struct GlwColorPalette {
    /// Stroke colour of area rectangle outlines.
    pub area_outline: image::Rgba<u8>,
    /// Stroke colour of circle outlines.
    pub circle_outline: image::Rgba<u8>,
    /// Stroke colour of the dashed margin band, when drawn.
    pub margin_outline: image::Rgba<u8>,
    /// Fill colour for filled wind arrows.
    pub wind_arrow: image::Rgba<u8>,
    /// Fill colour for filled current arrows.
    pub current_arrow: image::Rgba<u8>,
    /// Stroke colour for wave glyphs.
    pub wave_glyph: image::Rgba<u8>,
    /// Fill colour of the small centre marker on circles.
    pub center_marker: image::Rgba<u8>,
    /// Foreground colour for the per-area / per-circle text labels.
    pub label_fg: image::Rgba<u8>,
    /// Drop-shadow colour for the per-area / per-circle text labels.
    pub label_shadow: image::Rgba<u8>,
    /// Optional fill colour for area interiors. `None` means no fill
    /// (the default; matches the reference image).
    pub area_fill: Option<image::Rgba<u8>>,
    /// Background colour for the optional base legend panel.
    pub legend_bg: image::Rgba<u8>,
    /// Foreground colour for the legend panel content.
    pub legend_fg: image::Rgba<u8>,
}

impl Default for GlwColorPalette {
    fn default() -> Self {
        Self {
            area_outline: image::Rgba([40, 220, 40, 255]),
            circle_outline: image::Rgba([40, 220, 40, 255]),
            margin_outline: image::Rgba([40, 220, 40, 96]),
            wind_arrow: image::Rgba([255, 255, 255, 240]),
            current_arrow: image::Rgba([255, 255, 255, 200]),
            wave_glyph: image::Rgba([255, 255, 255, 220]),
            center_marker: image::Rgba([255, 255, 255, 200]),
            label_fg: image::Rgba([255, 255, 255, 255]),
            label_shadow: image::Rgba([0, 0, 0, 180]),
            area_fill: None,
            legend_bg: image::Rgba([0, 0, 0, 180]),
            legend_fg: image::Rgba([255, 255, 255, 240]),
        }
    }
}

/// Style knobs controlling how a [`crate::GlwEvent`] is rendered onto a
/// map.
#[expect(
    clippy::module_name_repetitions,
    reason = "GlwStyle is the primary public type of this module"
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag controls an independent render element; collapsing into a state machine would obscure intent"
)]
#[derive(Debug, Clone)]
pub struct GlwStyle {
    /// Colours used for each visual element.
    pub palette: GlwColorPalette,
    /// Pixels per knot of wind/current speed used to size arrows.
    pub arrow_pixels_per_knot: f32,
    /// Minimum arrow length in pixels, irrespective of speed.
    pub min_arrow_pixels: f32,
    /// Half-width of the filled arrow base in pixels.
    pub arrow_thickness_pixels: f32,
    /// Whether to draw text labels summarising the override values next
    /// to each area/circle.
    pub label_overrides: bool,
    /// Label font size in pixels.
    pub label_font_px: f32,
    /// Which placement slot the base wind/current/wave legend goes in, or
    /// `None` to skip it (the default).
    pub legend_position: Option<sl_map_apis::coverage::PlacementSlot>,
    /// Whether to draw a dashed outer rectangle/circle representing the
    /// margin band. Off by default; useful for diagnostic renders.
    pub draw_margin_band: bool,
    /// Pixel length of each "on" segment of the dashed margin band.
    pub margin_dash_pixels: f32,
    /// Pixel length of each "off" segment of the dashed margin band.
    pub margin_gap_pixels: f32,
    /// Whether to draw wave glyphs.
    pub draw_waves: bool,
    /// Whether to draw current arrows.
    pub draw_currents: bool,
    /// Density per axis of wind-arrow anchors inside a multi-sim area
    /// (`1` = one centred arrow, `2` = 2×2 grid, etc.). Circles always
    /// use a single arrow at the centre.
    pub area_arrow_density: u8,
}

impl Default for GlwStyle {
    fn default() -> Self {
        Self {
            palette: GlwColorPalette::default(),
            arrow_pixels_per_knot: 3.0,
            min_arrow_pixels: 16.0,
            arrow_thickness_pixels: 6.0,
            label_overrides: true,
            label_font_px: 14.0,
            legend_position: None,
            draw_margin_band: false,
            margin_dash_pixels: 6.0,
            margin_gap_pixels: 4.0,
            draw_waves: true,
            draw_currents: true,
            area_arrow_density: 2,
        }
    }
}
