//! Small text-drawing helpers used by label and legend rendering.
//!
//! The library does not bundle a font. Callers supply any
//! `ab_glyph::Font` they like — see the `_with_font` variants on
//! [`crate::MapLikeGlwExt`].

use ab_glyph::{Font, ScaleFont as _};
use sl_map_apis::map_tiles::MapLike;

/// Bundle of the parameters that every text-drawing call needs.
/// Keeps the public helpers below to a small number of arguments and
/// removes the temptation to pass colours, font and scale in different
/// orders at different call sites.
pub(crate) struct TextStyle<'a, F: Font> {
    /// Font borrowed from the caller; never owned by the library.
    pub font: &'a F,
    /// Pixel size to render glyphs at.
    pub scale: ab_glyph::PxScale,
    /// Foreground colour for the glyphs.
    pub fg: image::Rgba<u8>,
    /// Drop-shadow colour rendered one pixel down-right of the
    /// foreground.
    pub shadow: image::Rgba<u8>,
}

/// Draw `text` at integer pixel `(x, y)` (the top-left of the glyph
/// bounding box) with a single-pixel black drop shadow under the
/// foreground.
///
/// The shadow makes labels legible against both bright tile content
/// and dark water, matching the look of the GLW reference render.
pub(crate) fn draw_text_with_shadow<M, F>(
    map: &mut M,
    x: i32,
    y: i32,
    style: &TextStyle<'_, F>,
    text: &str,
) where
    M: MapLike + ?Sized,
    F: Font,
{
    // Drop shadow at (+1, +1). Drawn first so the foreground overlays it.
    imageproc::drawing::draw_text_mut(
        map.image_mut(),
        style.shadow,
        x + 1,
        y + 1,
        style.scale,
        style.font,
        text,
    );
    imageproc::drawing::draw_text_mut(
        map.image_mut(),
        style.fg,
        x,
        y,
        style.scale,
        style.font,
        text,
    );
}

/// Measure a multi-line text block: returns `(width_px, height_px)`,
/// where width is the widest single line and height is the total
/// stacked height of `lines.len()` rows at `scale`.
pub(crate) fn multi_line_size<F: Font>(
    scale: ab_glyph::PxScale,
    font: &F,
    lines: &[String],
) -> (u32, u32) {
    if lines.is_empty() {
        return (0, 0);
    }
    let scaled = font.as_scaled(scale);
    let line_height = scaled.height().ceil() as u32 + scaled.line_gap().ceil() as u32;
    let mut max_w: u32 = 0;
    for line in lines {
        let (w, _) = imageproc::drawing::text_size(scale, font, line);
        if w > max_w {
            max_w = w;
        }
    }
    let total_h = line_height * lines.len() as u32;
    (max_w, total_h)
}

/// Draw a sequence of lines stacked vertically starting at `(x, y)`
/// (top-left of the first line), each with [`draw_text_with_shadow`].
pub(crate) fn draw_multi_line_with_shadow<M, F>(
    map: &mut M,
    x: i32,
    y: i32,
    style: &TextStyle<'_, F>,
    lines: &[String],
) where
    M: MapLike + ?Sized,
    F: Font,
{
    let scaled = style.font.as_scaled(style.scale);
    let line_height = (scaled.height() + scaled.line_gap()).ceil() as i32;
    for (i, line) in lines.iter().enumerate() {
        let line_y = y + line_height * i as i32;
        draw_text_with_shadow(map, x, line_y, style, line);
    }
}
