//! Rendering of GLW overlays onto an existing `sl_map_apis` map.
//!
//! The public surface is the [`MapLikeGlwExt`] extension trait, which is
//! blanket-implemented for every `M: MapLike`. Callers can then write:
//!
//! ```no_run
//! use sl_glw::{GlwEvent, GlwStyle, MapLikeGlwExt};
//! # async fn demo<M: sl_map_apis::map_tiles::MapLike>(
//! #     mut map: M, event: &GlwEvent,
//! # ) -> Result<(), sl_glw::RenderError> {
//! map.draw_glw_event(event, &GlwStyle::default())?;
//! # Ok(()) }
//! ```
//!
//! Text rendering (per-shape labels and the corner legend) is opt-in
//! via the `_with_font` methods, which take a caller-supplied
//! `ab_glyph::Font`. The library deliberately does not bundle a font.

use ab_glyph::Font;
use sl_map_apis::map_tiles::MapLike;
use sl_types::map::{GridRectangleLike as _, RegionCoordinates};

use crate::error::RenderError;
use crate::geometry;
use crate::style::{GlwStyle, LegendPosition};
use crate::text;
use crate::types::{
    Area, Base, Circle, CurrentsOverride, GlwEvent, KnotSpeed, WaveHeight, WavesOverride,
    WindDirection, WindOverride,
};

/// Extension trait that adds GLW-overlay drawing methods to anything
/// that already implements [`MapLike`].
pub trait MapLikeGlwExt: MapLike {
    /// Draw a full GLW event onto the map (every area and every
    /// circle that intersects this map's grid rectangle), plus the
    /// optional base legend if the style requests it.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a rendering primitive fails. Note
    /// that out-of-range areas/circles are skipped silently with a
    /// `tracing::debug!`, not surfaced as an error.
    fn draw_glw_event(&mut self, event: &GlwEvent, style: &GlwStyle) -> Result<(), RenderError> {
        for area in event.areas.as_slice() {
            self.draw_glw_area(area, &event.base, style)?;
        }
        for circle in event.circles.as_slice() {
            self.draw_glw_circle(circle, &event.base, style)?;
        }
        // Base legend currently a no-op; see module docs.
        Ok(())
    }

    /// Draw a single area onto the map.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a rendering primitive fails.
    fn draw_glw_area(
        &mut self,
        area: &Area,
        base: &Base,
        style: &GlwStyle,
    ) -> Result<(), RenderError> {
        draw_area_default(self, area, base, style)
    }

    /// Draw a single circle onto the map.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a rendering primitive fails.
    fn draw_glw_circle(
        &mut self,
        circle: &Circle,
        base: &Base,
        style: &GlwStyle,
    ) -> Result<(), RenderError> {
        draw_circle_default(self, circle, base, style)
    }

    /// Like [`Self::draw_glw_event`], but additionally renders the
    /// per-shape override labels (if `style.label_overrides`) and the
    /// base legend panel (if `style.legend_position != None`) using the
    /// supplied font.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a rendering primitive fails.
    fn draw_glw_event_with_font<F: Font>(
        &mut self,
        event: &GlwEvent,
        style: &GlwStyle,
        font: &F,
    ) -> Result<(), RenderError> {
        self.draw_glw_event(event, style)?;
        if style.label_overrides {
            for area in event.areas.as_slice() {
                self.draw_glw_area_label(area, &event.base, style, font)?;
            }
            for circle in event.circles.as_slice() {
                self.draw_glw_circle_label(circle, &event.base, style, font)?;
            }
        }
        if style.legend_position != LegendPosition::None {
            self.draw_glw_base_legend(&event.base, style, font)?;
        }
        Ok(())
    }

    /// Draw the override-summary label for a single area, using the
    /// supplied font. Only the fields the area actually overrides are
    /// listed.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a rendering primitive fails.
    fn draw_glw_area_label<F: Font>(
        &mut self,
        area: &Area,
        base: &Base,
        style: &GlwStyle,
        font: &F,
    ) -> Result<(), RenderError> {
        draw_area_label_default(self, area, base, style, font)
    }

    /// Draw the override-summary label for a single circle, using the
    /// supplied font. Only the fields the circle actually overrides are
    /// listed.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a rendering primitive fails.
    fn draw_glw_circle_label<F: Font>(
        &mut self,
        circle: &Circle,
        base: &Base,
        style: &GlwStyle,
        font: &F,
    ) -> Result<(), RenderError> {
        draw_circle_label_default(self, circle, base, style, font)
    }

    /// Draw the base wind/current/wave legend in the corner of the map
    /// specified by `style.legend_position`. No-op if the position is
    /// [`LegendPosition::None`].
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a rendering primitive fails.
    fn draw_glw_base_legend<F: Font>(
        &mut self,
        base: &Base,
        style: &GlwStyle,
        font: &F,
    ) -> Result<(), RenderError> {
        draw_base_legend_default(self, base, style, font)
    }
}

impl<M: MapLike + ?Sized> MapLikeGlwExt for M {}

/// Convenience free function: same as `MapLikeGlwExt::draw_glw_event`.
///
/// # Errors
///
/// Returns [`RenderError`] if a rendering primitive fails.
pub fn draw_glw_event<M: MapLike + ?Sized>(
    map: &mut M,
    event: &GlwEvent,
    style: &GlwStyle,
) -> Result<(), RenderError> {
    map.draw_glw_event(event, style)
}

/// Default implementation of `draw_glw_area` (factored out so the
/// blanket impl stays a no-op forwarding shell).
fn draw_area_default<M: MapLike + ?Sized>(
    map: &mut M,
    area: &Area,
    base: &Base,
    style: &GlwStyle,
) -> Result<(), RenderError> {
    let Some((sw_x, sw_y)) = map.pixel_coordinates_for_coordinates(
        &area.grid_rectangle.lower_left_corner(),
        &RegionCoordinates::new(0.0, 0.0, 0.0),
    ) else {
        tracing::debug!(area = %area.name, "area outside map; skipping");
        return Ok(());
    };
    let Some((ne_x, ne_y)) = map.pixel_coordinates_for_coordinates(
        &area.grid_rectangle.upper_right_corner(),
        &RegionCoordinates::new(256.0, 256.0, 0.0),
    ) else {
        tracing::debug!(area = %area.name, "area corner outside map; skipping");
        return Ok(());
    };
    let (x0, x1) = sort_pair(sw_x, ne_x);
    let (y0, y1) = sort_pair(sw_y, ne_y);
    let width = x1.saturating_sub(x0).max(1);
    let height = y1.saturating_sub(y0).max(1);
    let rect = imageproc::rect::Rect::at(
        i32::try_from(x0).unwrap_or(0),
        i32::try_from(y0).unwrap_or(0),
    )
    .of_size(width, height);

    if let Some(fill) = style.palette.area_fill {
        imageproc::drawing::draw_filled_rect_mut(map.image_mut(), rect, fill);
    }
    imageproc::drawing::draw_hollow_rect_mut(map.image_mut(), rect, style.palette.area_outline);

    // Margin band — dashed rectangle inflated outward by the override's
    // `margin` (in meters). Marks where the override starts blending
    // into base conditions; the solid green outline marks where the
    // override is fully applied.
    if style.draw_margin_band {
        let margin_m = f32::from(area.margin.meters());
        if margin_m > 0.0 {
            let margin_px = margin_m * map.pixels_per_meter();
            geometry::dashed_rect(
                map,
                (x0 as f32 - margin_px, y0 as f32 - margin_px),
                (x1 as f32 + margin_px, y1 as f32 + margin_px),
                style.margin_dash_pixels,
                style.margin_gap_pixels,
                style.palette.margin_outline,
            );
        }
    }

    let wind_dir = area
        .wind
        .and_then(|w| w.direction)
        .unwrap_or(base.wind.direction);
    let wind_speed = area.wind.and_then(|w| w.speed).unwrap_or(base.wind.speed);
    tracing::debug!(
        area = %area.name,
        wind = geometry::degrees_to_compass8(wind_dir),
        wind_knots = wind_speed.knots(),
        "drawing area"
    );

    // Wind arrows on a regular grid inside the rectangle so a large
    // area shows the wind across its whole footprint instead of one
    // glyph at the centroid.
    let density = u32::from(style.area_arrow_density.max(1));
    let fx0 = x0 as f32;
    let fy0 = y0 as f32;
    let fw = (x1 - x0) as f32;
    let fh = (y1 - y0) as f32;
    for i_x in 0..density {
        for i_y in 0..density {
            let u = (i_x as f32 + 0.5) / density as f32;
            let v = (i_y as f32 + 0.5) / density as f32;
            let anchor = (fx0 + u * fw, fy0 + v * fh);
            draw_wind_glyph(map, anchor, wind_dir, wind_speed, style);
        }
    }

    // Anchors for currents / waves: chosen so they don't sit underneath
    // a wind-arrow grid cell. Currents go in the top-right inset,
    // waves in the bottom-left inset. Insets scale with the smaller
    // side so the glyphs stay inside even on narrow shapes.
    let inset = (fw.min(fh) * 0.18).clamp(14.0, 40.0);
    if let (true, Some(currents)) = (style.draw_currents, area.currents) {
        let speed = currents.speed.unwrap_or(base.currents.speed);
        let dir = currents.direction.unwrap_or(base.currents.direction);
        let anchor = (fx0 + fw - inset, fy0 + inset);
        draw_current_glyph(map, anchor, dir, speed, style);
    }
    if let (true, Some(waves)) = (style.draw_waves, area.waves) {
        let height_m = waves.height.unwrap_or(base.waves.height);
        let anchor = (fx0 + inset, fy0 + fh - inset);
        draw_wave_glyph(map, anchor, height_m.meters(), style);
    }
    Ok(())
}

/// Default implementation of `draw_glw_circle`.
fn draw_circle_default<M: MapLike + ?Sized>(
    map: &mut M,
    circle: &Circle,
    base: &Base,
    style: &GlwStyle,
) -> Result<(), RenderError> {
    let Some(center) = geometry::circle_center_pixel(map, &circle.center_sim, &circle.center_point)
    else {
        tracing::debug!(circle = %circle.name, "circle centre outside map; skipping");
        return Ok(());
    };
    let radius_px = (circle.radius.meters() * map.pixels_per_meter()).max(1.0);
    let cx_i = i32::try_from(center.0 as i64).unwrap_or(0);
    let cy_i = i32::try_from(center.1 as i64).unwrap_or(0);
    imageproc::drawing::draw_hollow_circle_mut(
        map.image_mut(),
        (cx_i, cy_i),
        radius_px as i32,
        style.palette.circle_outline,
    );

    // Margin band — dashed outer circle at radius + margin_px. See the
    // matching block in the area render path for what this represents.
    if style.draw_margin_band {
        let margin_m = f32::from(circle.margin.meters());
        if margin_m > 0.0 {
            let margin_px = margin_m * map.pixels_per_meter();
            geometry::dashed_circle(
                map,
                center,
                radius_px + margin_px,
                style.margin_dash_pixels,
                style.margin_gap_pixels,
                style.palette.margin_outline,
            );
        }
    }

    // No centre marker: the wind arrow at the centre already locates
    // the circle, and the reference image (`example-map-glw.png`) does
    // not draw one. `palette.center_marker` is kept on `GlwStyle` for
    // future use (e.g. by callers that want to mark very small circles
    // that don't show a wind arrow).
    let wind_dir = circle
        .wind
        .and_then(|w| w.direction)
        .unwrap_or(base.wind.direction);
    let wind_speed = circle.wind.and_then(|w| w.speed).unwrap_or(base.wind.speed);
    draw_wind_glyph(map, center, wind_dir, wind_speed, style);

    // Offset distance kept inside the circle: ~55 % of the radius. The
    // current and wave glyphs sit on opposite diagonals so they share
    // no pixels with the wind arrow at the centre.
    let offset = (radius_px * 0.55).clamp(18.0, 60.0);
    if let (true, Some(currents)) = (style.draw_currents, circle.currents) {
        let speed = currents.speed.unwrap_or(base.currents.speed);
        let dir = currents.direction.unwrap_or(base.currents.direction);
        let anchor = (center.0 + offset, center.1 - offset);
        draw_current_glyph(map, anchor, dir, speed, style);
    }
    if let (true, Some(waves)) = (style.draw_waves, circle.waves) {
        let height_m = waves.height.unwrap_or(base.waves.height);
        let anchor = (center.0 - offset, center.1 + offset);
        draw_wave_glyph(map, anchor, height_m.meters(), style);
    }
    Ok(())
}

/// Sort two unsigned pixel coordinates so the lower value comes first.
const fn sort_pair(a: u32, b: u32) -> (u32, u32) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Compute the arrow shaft length in pixels from a speed and the style's
/// pixels-per-knot scaling, clamped to the configured minimum.
fn arrow_length_px(speed: KnotSpeed, style: &GlwStyle) -> f32 {
    (speed.knots() * style.arrow_pixels_per_knot).max(style.min_arrow_pixels)
}

/// Stroke a single line segment using floating-point endpoints.
///
/// Wraps `imageproc::drawing::draw_line_segment_mut` so the rest of this
/// module reads as drawing primitives instead of imageproc machinery.
fn stroke_line<M: MapLike + ?Sized>(
    map: &mut M,
    a: (f32, f32),
    b: (f32, f32),
    color: image::Rgba<u8>,
) {
    imageproc::drawing::draw_line_segment_mut(map.image_mut(), a, b, color);
}

/// Draw the wind glyph: shaft + tail crossbar + chevron arrowhead at the
/// tip, all in outline strokes. Matches the look of the reference image
/// (`example-map-glw.png`) — a stylised wind barb pointing in the
/// "blowing toward" direction.
fn draw_wind_glyph<M: MapLike + ?Sized>(
    map: &mut M,
    center: (f32, f32),
    dir: WindDirection,
    speed: KnotSpeed,
    style: &GlwStyle,
) {
    let length = arrow_length_px(speed, style);
    let (dx, dy) = geometry::blowing_toward_unit_vec(dir);
    // Perpendicular unit vector (image space, +y down). Right-hand
    // perpendicular of (dx, dy) is (-dy, dx).
    let (px, py) = (-dy, dx);
    let half = length * 0.5;
    let tail = (center.0 - dx * half, center.1 - dy * half);
    let tip = (center.0 + dx * half, center.1 + dy * half);
    let color = style.palette.wind_arrow;

    // Shaft.
    stroke_line(map, tail, tip, color);

    // Tail fin — a single short perpendicular line off the LEFT side
    // of the tail (looking in the direction the arrow points). Not a
    // centred crossbar; matches the reference image.
    //
    // `(px, py) = (-dy, dx)` is the right-perpendicular in image
    // space; subtracting it goes to the left.
    let tail_fin_len = (length * 0.22).clamp(4.0, 10.0);
    let tail_fin_end = (tail.0 - px * tail_fin_len, tail.1 - py * tail_fin_len);
    stroke_line(map, tail, tail_fin_end, color);

    // Filled chevron arrowhead — a triangle with the back two corners
    // pulled further back along the shaft and a notch cut into the
    // middle of the back. Renders as a filled 4-point polygon:
    // `tip → back-right corner → centre notch → back-left corner`.
    let head_back = (length * 0.36).clamp(6.0, 14.0);
    let head_notch = (length * 0.20).clamp(3.0, 8.0);
    let head_side = (length * 0.22).clamp(4.0, 10.0);
    let back_right = (
        tip.0 - dx * head_back - px * head_side,
        tip.1 - dy * head_back - py * head_side,
    );
    let back_left = (
        tip.0 - dx * head_back + px * head_side,
        tip.1 - dy * head_back + py * head_side,
    );
    let notch = (tip.0 - dx * head_notch, tip.1 - dy * head_notch);
    let p_tip = imageproc::point::Point::<i32>::new(tip.0 as i32, tip.1 as i32);
    let p_back_right =
        imageproc::point::Point::<i32>::new(back_right.0 as i32, back_right.1 as i32);
    let p_notch = imageproc::point::Point::<i32>::new(notch.0 as i32, notch.1 as i32);
    let p_back_left = imageproc::point::Point::<i32>::new(back_left.0 as i32, back_left.1 as i32);
    // draw_polygon_mut requires distinct consecutive vertices; collapse
    // gracefully when the head is too small to be expressible in
    // integer pixels.
    if p_tip != p_back_right
        && p_back_right != p_notch
        && p_notch != p_back_left
        && p_back_left != p_tip
    {
        imageproc::drawing::draw_polygon_mut(
            map.image_mut(),
            &[p_tip, p_back_right, p_notch, p_back_left],
            color,
        );
    }
}

/// Draw the current glyph: a straight shaft with a small filled
/// triangular head at the tip whose back edge is flat (perpendicular
/// to the shaft). Both shaft and head scale with the computed length
/// so the shaft stays visible past the head at any speed.
fn draw_current_glyph<M: MapLike + ?Sized>(
    map: &mut M,
    center: (f32, f32),
    dir: WindDirection,
    speed: KnotSpeed,
    style: &GlwStyle,
) {
    // Current arrows render at 2x the base arrow length so they read
    // clearly even at the small speeds typical for currents (often
    // <2 knots). Head clamps scale with the longer shaft.
    let length = arrow_length_px(speed, style) * 2.0;
    let (dx, dy) = geometry::blowing_toward_unit_vec(dir);
    let (px, py) = (-dy, dx);
    let half = length * 0.5;
    let tail = (center.0 - dx * half, center.1 - dy * half);
    let tip = (center.0 + dx * half, center.1 + dy * half);
    let color = style.palette.current_arrow;

    let head_len = (length * 0.30).clamp(8.0, 18.0);
    let head_half = head_len * 0.55;
    let head_base = (tip.0 - dx * head_len, tip.1 - dy * head_len);

    // Shaft goes from tail to where the triangle base sits so the head
    // sits cleanly on the end of the shaft.
    stroke_line(map, tail, head_base, color);

    let p_tip = imageproc::point::Point::<i32>::new(tip.0 as i32, tip.1 as i32);
    let p_left = imageproc::point::Point::<i32>::new(
        (head_base.0 + px * head_half) as i32,
        (head_base.1 + py * head_half) as i32,
    );
    let p_right = imageproc::point::Point::<i32>::new(
        (head_base.0 - px * head_half) as i32,
        (head_base.1 - py * head_half) as i32,
    );
    // draw_polygon_mut panics on duplicate consecutive points or a
    // degenerate triangle. Bail in the degenerate cases instead.
    if p_tip != p_left && p_tip != p_right && p_left != p_right {
        imageproc::drawing::draw_polygon_mut(map.image_mut(), &[p_tip, p_left, p_right], color);
    }
}

/// Format a knot speed as a short string: integer if it rounds cleanly
/// (`"17"`), otherwise to one decimal place (`"1.5"`). Matches the
/// terse format used in the reference image's labels.
fn format_speed(speed: KnotSpeed) -> String {
    let k = speed.knots();
    if (k - k.round()).abs() < 0.05 {
        format!("{k:.0}")
    } else {
        format!("{k:.1}")
    }
}

/// Format a wave height in meters as `"2.0m"` (always one decimal so
/// integer-valued heights still read as a measurement).
fn format_height(h: WaveHeight) -> String {
    format!("{:.1}m", h.meters())
}

/// Build the multi-line label for an area or circle from its override
/// blocks. Each block contributes one line if non-empty; an additional
/// effects line is appended for waves overrides that name effects.
fn build_label_lines(
    base: &Base,
    wind: Option<WindOverride>,
    currents: Option<CurrentsOverride>,
    waves: Option<WavesOverride>,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(w) = wind.filter(|w| !w.is_empty()) {
        let dir = w.direction.unwrap_or(base.wind.direction);
        let speed = w.speed.unwrap_or(base.wind.speed);
        lines.push(format!(
            "Wind: {} {}Kt",
            geometry::degrees_to_compass8(dir),
            format_speed(speed),
        ));
    }
    if let Some(c) = currents.filter(|c| !c.is_empty()) {
        let dir = c.direction.unwrap_or(base.currents.direction);
        let speed = c.speed.unwrap_or(base.currents.speed);
        lines.push(format!(
            "Current: {} {}Kt",
            geometry::degrees_to_compass8(dir),
            format_speed(speed),
        ));
    }
    if let Some(wv) = waves.filter(|w| !w.is_empty()) {
        let height = wv.height.unwrap_or(base.waves.height);
        lines.push(format!("Waves: {}", format_height(height)));
        if let Some(effects) = wv.effects {
            let label = match (effects.speed, effects.steer) {
                (true, true) => "spd+steer",
                (true, false) => "spd",
                (false, true) => "steer",
                (false, false) => "",
            };
            if !label.is_empty() {
                lines.push(label.to_owned());
            }
        }
    }
    lines
}

/// Build the always-three-line label for the base block: wind, current,
/// waves. Used by the corner legend panel.
fn build_base_legend_lines(base: &Base) -> Vec<String> {
    vec![
        format!(
            "Wind: {} {}Kt",
            geometry::degrees_to_compass8(base.wind.direction),
            format_speed(base.wind.speed),
        ),
        format!(
            "Current: {} {}Kt",
            geometry::degrees_to_compass8(base.currents.direction),
            format_speed(base.currents.speed),
        ),
        format!("Waves: {}", format_height(base.waves.height)),
    ]
}

/// Default implementation of `draw_glw_area_label`: places the label
/// inside the rectangle, horizontally centred at the bottom edge.
fn draw_area_label_default<M, F>(
    map: &mut M,
    area: &Area,
    base: &Base,
    style: &GlwStyle,
    font: &F,
) -> Result<(), RenderError>
where
    M: MapLike + ?Sized,
    F: Font,
{
    let lines = build_label_lines(base, area.wind, area.currents, area.waves);
    if lines.is_empty() {
        return Ok(());
    }
    let Some((sw_x, sw_y)) = map.pixel_coordinates_for_coordinates(
        &area.grid_rectangle.lower_left_corner(),
        &RegionCoordinates::new(0.0, 0.0, 0.0),
    ) else {
        return Ok(());
    };
    let Some((ne_x, ne_y)) = map.pixel_coordinates_for_coordinates(
        &area.grid_rectangle.upper_right_corner(),
        &RegionCoordinates::new(256.0, 256.0, 0.0),
    ) else {
        return Ok(());
    };
    let (x0, x1) = sort_pair(sw_x, ne_x);
    let (y0, y1) = sort_pair(sw_y, ne_y);
    let scale = ab_glyph::PxScale::from(style.label_font_px);
    let (text_w, text_h) = text::multi_line_size(scale, font, &lines);
    let inset_y = 6_i32;
    let centre_x = x0.midpoint(x1);
    let x = i32::try_from(centre_x).unwrap_or(0) - i32::try_from(text_w / 2).unwrap_or(0);
    let y = i32::try_from(y1).unwrap_or(0) - i32::try_from(text_h).unwrap_or(0) - inset_y;
    let y = y.max(i32::try_from(y0).unwrap_or(0) + inset_y);
    let text_style = text::TextStyle {
        font,
        scale,
        fg: style.palette.label_fg,
        shadow: style.palette.label_shadow,
    };
    text::draw_multi_line_with_shadow(map, x, y, &text_style, &lines);
    Ok(())
}

/// Default implementation of `draw_glw_circle_label`: places the label
/// inside the circle, centred horizontally just above the bottom edge.
fn draw_circle_label_default<M, F>(
    map: &mut M,
    circle: &Circle,
    base: &Base,
    style: &GlwStyle,
    font: &F,
) -> Result<(), RenderError>
where
    M: MapLike + ?Sized,
    F: Font,
{
    let lines = build_label_lines(base, circle.wind, circle.currents, circle.waves);
    if lines.is_empty() {
        return Ok(());
    }
    let Some(center) = geometry::circle_center_pixel(map, &circle.center_sim, &circle.center_point)
    else {
        return Ok(());
    };
    let radius_px = (circle.radius.meters() * map.pixels_per_meter()).max(1.0);
    let scale = ab_glyph::PxScale::from(style.label_font_px);
    let (text_w, text_h) = text::multi_line_size(scale, font, &lines);
    let diameter = 2.0 * radius_px;
    let inset = 4.0_f32;
    // Place the label inside the circle if it comfortably fits (with a
    // 0.8 fudge factor so it doesn't crowd the outline); otherwise put
    // it just below the circle so it stays readable on small circles.
    let fits_inside = text_w as f32 <= diameter * 0.8 && text_h as f32 <= diameter * 0.6;
    let x = (center.0 - text_w as f32 * 0.5) as i32;
    let y = if fits_inside {
        (center.1 + radius_px - text_h as f32 - inset) as i32
    } else {
        (center.1 + radius_px + inset) as i32
    };
    let text_style = text::TextStyle {
        font,
        scale,
        fg: style.palette.label_fg,
        shadow: style.palette.label_shadow,
    };
    text::draw_multi_line_with_shadow(map, x, y, &text_style, &lines);
    Ok(())
}

/// Default implementation of `draw_glw_base_legend`: draws a small
/// translucent panel in the configured corner with the base wind /
/// current / wave summary on it.
fn draw_base_legend_default<M, F>(
    map: &mut M,
    base: &Base,
    style: &GlwStyle,
    font: &F,
) -> Result<(), RenderError>
where
    M: MapLike + ?Sized,
    F: Font,
{
    if style.legend_position == LegendPosition::None {
        return Ok(());
    }
    let lines = build_base_legend_lines(base);
    let scale = ab_glyph::PxScale::from(style.label_font_px);
    let (text_w, text_h) = text::multi_line_size(scale, font, &lines);
    let pad: u32 = 6;
    let panel_w = text_w + pad * 2;
    let panel_h = text_h + pad * 2;
    let (img_w, img_h) = image::GenericImageView::dimensions(map.image());
    let edge_gap: u32 = 8;
    let (px_origin, py_origin) = match style.legend_position {
        LegendPosition::TopLeft => (edge_gap, edge_gap),
        LegendPosition::TopRight => (img_w.saturating_sub(panel_w + edge_gap), edge_gap),
        LegendPosition::BottomLeft => (edge_gap, img_h.saturating_sub(panel_h + edge_gap)),
        LegendPosition::BottomRight => (
            img_w.saturating_sub(panel_w + edge_gap),
            img_h.saturating_sub(panel_h + edge_gap),
        ),
        LegendPosition::None => return Ok(()),
    };
    let rect = imageproc::rect::Rect::at(
        i32::try_from(px_origin).unwrap_or(0),
        i32::try_from(py_origin).unwrap_or(0),
    )
    .of_size(panel_w.max(1), panel_h.max(1));
    imageproc::drawing::draw_filled_rect_mut(map.image_mut(), rect, style.palette.legend_bg);
    let text_style = text::TextStyle {
        font,
        scale,
        fg: style.palette.legend_fg,
        shadow: style.palette.label_shadow,
    };
    text::draw_multi_line_with_shadow(
        map,
        i32::try_from(px_origin + pad).unwrap_or(0),
        i32::try_from(py_origin + pad).unwrap_or(0),
        &text_style,
        &lines,
    );
    Ok(())
}

/// Draw the wave glyph: two stacked sine-like rows, each with two
/// crests and a trough between them, centred on `center`. Glyph width
/// and crest amplitude scale gently with wave height.
fn draw_wave_glyph<M: MapLike + ?Sized>(
    map: &mut M,
    center: (f32, f32),
    height_m: f32,
    style: &GlwStyle,
) {
    let amplitude = (1.5 + height_m * 0.8).clamp(2.5, 5.0);
    let half_width = (10.0 + height_m * 1.5).clamp(12.0, 22.0);
    let row_spacing = amplitude * 2.0 + 3.0;
    let color = style.palette.wave_glyph;
    // Two stacked rows centred vertically on `center`. Each row is a
    // two-period cosine — phase sweep 0..4π — so the row reads as
    // crest-trough-crest-trough-crest from left to right (matches the
    // reference image: high ends, high middle, two troughs between).
    for row in [-1.0_f32, 1.0] {
        let row_y = center.1 + row * row_spacing * 0.5;
        let segments: u32 = 24;
        let mut prev: Option<(f32, f32)> = None;
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let x = center.0 - half_width + 2.0 * half_width * t;
            let phase = 4.0 * std::f32::consts::PI * t;
            let y = row_y - amplitude * phase.cos();
            if let Some(p) = prev {
                stroke_line(map, p, (x, y), color);
            }
            prev = Some((x, y));
        }
    }
}
