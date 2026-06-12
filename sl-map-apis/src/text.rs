//! Text-drawing helpers usable on any [`MapLike`]: measuring multi-line text
//! and drawing free-floating labels with a drop shadow. These are not tied to
//! any overlay type — the GLW overlay renderer and the placement-label
//! renderer both build on them.
//!
//! The library does not bundle a font; callers supply any `ab_glyph::Font`.

use std::path::{Path, PathBuf};

use ab_glyph::{Font, ScaleFont as _};

use crate::map_tiles::MapLike;

/// Errors from [`load_font`].
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// the font file could not be read
    #[error("could not read font file `{path}`: {source}")]
    Read {
        /// the path that failed to read
        path: PathBuf,
        /// the underlying IO error
        source: std::io::Error,
    },
    /// the bytes could not be parsed as a TrueType/OpenType font
    #[error("could not parse font file as a TrueType/OpenType font: {0}")]
    Parse(#[from] ab_glyph::InvalidFont),
}

/// Load and parse a TrueType/OpenType font file into an owned
/// [`ab_glyph::FontVec`]. The path is included in the error on a read
/// failure.
///
/// # Errors
///
/// Returns [`FontError::Read`] if the file cannot be read, or
/// [`FontError::Parse`] if it is not a valid font.
pub fn load_font(path: &Path) -> Result<ab_glyph::FontVec, FontError> {
    let bytes = std::fs::read(path).map_err(|source| FontError::Read {
        path: path.to_owned(),
        source,
    })?;
    Ok(ab_glyph::FontVec::try_from_vec(bytes)?)
}

/// Derive a friendly display name from a font's file name by stripping the
/// extension and replacing `-`/`_` with spaces (`noto-sans-mono.ttf` →
/// `noto sans mono`). Used as a fallback when a font has no usable embedded
/// name (see [`embedded_font_name`]).
#[must_use]
pub fn display_name_from_file_name(file_name: &str) -> String {
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    stem.replace(['-', '_'], " ")
}

/// Extract a human-readable name from a font's OpenType `name` table.
/// Prefers the full font name (id 4), then the typographic family (id 16),
/// then the legacy family (id 1). Returns `None` when the bytes cannot be
/// parsed or have no decodable, non-empty record (the caller can then fall
/// back to [`display_name_from_file_name`]).
#[must_use]
pub fn embedded_font_name(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let names = face.names();
    // Name IDs from the OpenType `name` table, in display preference.
    for want in [4u16, 16, 1] {
        for i in 0..names.len() {
            let Some(name) = names.get(i) else { continue };
            if name.name_id != want || !name.is_unicode() {
                continue;
            }
            // `to_string` decodes Windows/Unicode UTF-16BE records and
            // returns `None` for encodings it cannot handle.
            if let Some(decoded) = name.to_string() {
                let trimmed = decoded.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }
    None
}

/// Styling for a text label drawn via [`MapLike::draw_text_label`]: the pixel
/// size plus foreground and drop-shadow colours. The font is passed separately
/// so the caller keeps ownership of it.
#[derive(Debug, Clone, Copy)]
pub struct LabelStyle {
    /// Pixel size to render glyphs at.
    pub scale: ab_glyph::PxScale,
    /// Foreground colour for the glyphs.
    pub fg: image::Rgba<u8>,
    /// Drop-shadow colour rendered one pixel down-right of the foreground.
    pub shadow: image::Rgba<u8>,
    /// Horizontal alignment of each line within the multi-line block (the block
    /// is as wide as its widest line). Single-line labels are unaffected.
    pub align: crate::coverage::HAlign,
}

/// Measure the rendered pixel size `(width, height)` of a multi-line text
/// block at the given font and pixel size, where width is the widest line and
/// height is the total stacked height of `lines.len()` rows. Use this to check
/// whether a label will fit a target area before drawing it.
#[must_use]
#[expect(
    clippy::module_name_repetitions,
    reason = "measure_text reads naturally at call sites; the text module groups text helpers"
)]
pub fn measure_text<F: Font>(scale: ab_glyph::PxScale, font: &F, lines: &[String]) -> (u32, u32) {
    if lines.is_empty() {
        return (0, 0);
    }
    let scaled = font.as_scaled(scale);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "font line metrics for a sane pixel size are small positive values, nowhere near u32::MAX"
    )]
    let line_height = scaled.height().ceil() as u32 + scaled.line_gap().ceil() as u32;
    let mut max_w: u32 = 0;
    for line in lines {
        let (w, _) = imageproc::drawing::text_size(scale, font, line);
        if w > max_w {
            max_w = w;
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "line counts for any real label are tiny, nowhere near u32::MAX"
    )]
    let total_h = line_height * lines.len() as u32;
    (max_w, total_h)
}

/// Draw `text` at integer pixel `(x, y)` (top-left of the glyph bounding box)
/// with a single-pixel drop shadow under the foreground, for legibility on
/// varied tile backgrounds.
pub(crate) fn draw_text_with_shadow<M, F>(
    map: &mut M,
    x: i32,
    y: i32,
    style: &LabelStyle,
    font: &F,
    text: &str,
) where
    M: MapLike + ?Sized,
    F: Font,
{
    // Shadow at (+1, +1), drawn first so the foreground overlays it.
    imageproc::drawing::draw_text_mut(
        map.image_mut(),
        style.shadow,
        x + 1,
        y + 1,
        style.scale,
        font,
        text,
    );
    imageproc::drawing::draw_text_mut(map.image_mut(), style.fg, x, y, style.scale, font, text);
}

/// Draw a sequence of lines stacked vertically starting at `(x, y)` (top-left
/// of the first line), each with [`draw_text_with_shadow`].
pub(crate) fn draw_multi_line_with_shadow<M, F>(
    map: &mut M,
    x: i32,
    y: i32,
    style: &LabelStyle,
    font: &F,
    lines: &[String],
) where
    M: MapLike + ?Sized,
    F: Font,
{
    let scaled = font.as_scaled(style.scale);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "font line metrics for a sane pixel size are small positive values, nowhere near i32::MAX"
    )]
    let line_height = (scaled.height() + scaled.line_gap()).ceil() as i32;
    // Block width is the widest line; each line is aligned within it so the
    // per-line alignment is independent of the block's placement in the slot.
    let mut block_w: u32 = 0;
    for line in lines {
        let (w, _) = imageproc::drawing::text_size(style.scale, font, line);
        if w > block_w {
            block_w = w;
        }
    }
    for (i, line) in lines.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            reason = "line counts for any real label are tiny, nowhere near i32::MAX"
        )]
        let line_y = y + line_height * i as i32;
        let (line_w, _) = imageproc::drawing::text_size(style.scale, font, line);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "the alignment offset is a small pixel value, nowhere near i32::MAX"
        )]
        let x_off = style.align.offset(line_w, block_w) as i32;
        draw_text_with_shadow(map, x + x_off, line_y, style, font, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_tiles::Map;
    use sl_types::map::{GridCoordinates, GridRectangle, ZoomLevel};

    /// The font checked in at the workspace root, used to exercise the
    /// `name`-table extraction offline.
    const DEJAVU: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../DejaVuSans.ttf"));

    #[test]
    fn display_name_drops_extension_and_replaces_separators() {
        assert_eq!(display_name_from_file_name("DejaVuSans.ttf"), "DejaVuSans");
        assert_eq!(
            display_name_from_file_name("noto-sans-mono.ttf"),
            "noto sans mono"
        );
        assert_eq!(
            display_name_from_file_name("source_code_pro.ttf"),
            "source code pro"
        );
    }

    #[test]
    fn embedded_name_extracts_full_name() {
        assert_eq!(embedded_font_name(DEJAVU).as_deref(), Some("DejaVu Sans"));
    }

    #[test]
    fn embedded_name_rejects_garbage() {
        assert_eq!(embedded_font_name(b"not a font"), None);
    }

    #[test]
    fn load_font_reads_and_parses() -> Result<(), Box<dyn std::error::Error>> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../DejaVuSans.ttf");
        load_font(std::path::Path::new(path))?;
        // a missing file is a read error, not a parse error
        assert!(matches!(
            load_font(std::path::Path::new("/no/such/font.ttf")),
            Err(FontError::Read { .. })
        ));
        Ok(())
    }

    /// Load the repository's bundled DejaVuSans font for text tests.
    fn test_font() -> Result<ab_glyph::FontVec, Box<dyn std::error::Error>> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../DejaVuSans.ttf");
        Ok(ab_glyph::FontVec::try_from_vec(std::fs::read(path)?)?)
    }

    /// A blank 352x352 RGBA map (11 sims at zoom 4) for drawing tests.
    fn blank_map() -> Result<Map, Box<dyn std::error::Error>> {
        let grid = GridRectangle::new(
            GridCoordinates::new(1130, 1130),
            GridCoordinates::new(1140, 1140),
        );
        Ok(Map::blank(grid, ZoomLevel::try_new(4)?))
    }

    #[test]
    fn measure_text_stacks_lines() -> Result<(), Box<dyn std::error::Error>> {
        let font = test_font()?;
        let scale = ab_glyph::PxScale::from(16.0);
        let one = measure_text(scale, &font, &["Hello".to_owned()]);
        let two = measure_text(scale, &font, &["Hello".to_owned(), "Worldly".to_owned()]);
        assert!(one.0 > 0 && one.1 > 0);
        // two lines are exactly twice as tall and at least as wide
        assert_eq!(two.1, one.1 * 2);
        assert!(two.0 >= one.0);
        // no lines measures to nothing
        assert_eq!(measure_text(scale, &font, &[]), (0, 0));
        Ok(())
    }

    #[test]
    fn draw_text_label_writes_pixels_near_origin() -> Result<(), Box<dyn std::error::Error>> {
        let font = test_font()?;
        let mut map = blank_map()?;
        let style = LabelStyle {
            scale: ab_glyph::PxScale::from(20.0),
            fg: image::Rgba([255, 255, 255, 255]),
            shadow: image::Rgba([0, 0, 0, 200]),
            align: crate::coverage::HAlign::Left,
        };
        map.draw_text_label((40, 40), &["Label".to_owned()], &style, &font);
        // some pixel in the label region must now be non-transparent
        let mut drawn = false;
        for y in 40..80u32 {
            for x in 40..200u32 {
                let image::Rgba([_, _, _, a]) = image::GenericImageView::get_pixel(&map, x, y);
                if a != 0 {
                    drawn = true;
                }
            }
        }
        assert!(drawn, "draw_text_label should write visible pixels");
        Ok(())
    }

    #[test]
    fn multi_line_aligns_each_line_within_the_block() -> Result<(), Box<dyn std::error::Error>> {
        use crate::coverage::HAlign;
        let font = test_font()?;
        let scale = ab_glyph::PxScale::from(24.0);
        // a short line stacked over a wide line: the short line's horizontal
        // position within the block depends on the per-line alignment.
        let lines = vec!["I".to_owned(), "WWWWWWWW".to_owned()];
        let scaled = font.as_scaled(scale);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "test"
        )]
        let line_h = (scaled.height() + scaled.line_gap()).ceil() as u32;

        // Leftmost non-transparent x across the first line's rows.
        let first_line_min_x = |map: &Map| -> Option<u32> {
            let mut found: Option<u32> = None;
            for y in 10..(10 + line_h) {
                for x in 0..352u32 {
                    let image::Rgba([_, _, _, a]) = image::GenericImageView::get_pixel(map, x, y);
                    if a != 0 {
                        found = Some(found.map_or(x, |m| m.min(x)));
                    }
                }
            }
            found
        };

        let mut left = blank_map()?;
        let left_style = LabelStyle {
            scale,
            fg: image::Rgba([255, 255, 255, 255]),
            shadow: image::Rgba([0, 0, 0, 200]),
            align: HAlign::Left,
        };
        left.draw_text_label((10, 10), &lines, &left_style, &font);

        let mut right = blank_map()?;
        let right_style = LabelStyle {
            align: HAlign::Right,
            ..left_style
        };
        right.draw_text_label((10, 10), &lines, &right_style, &font);

        let left_min = first_line_min_x(&left).ok_or("left first line drew nothing")?;
        let right_min = first_line_min_x(&right).ok_or("right first line drew nothing")?;
        assert!(
            right_min > left_min + 10,
            "the short first line should shift right under right alignment (left={left_min}, right={right_min})"
        );
        Ok(())
    }
}
