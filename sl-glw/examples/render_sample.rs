//! Render the documented sample GLW event onto a black background and
//! save the result to `target/sl-glw-sample.png` for visual inspection.
//!
//! Takes one required positional argument: the path to a TrueType font
//! file used for the labels and the corner legend. The library never
//! bundles a font of its own — every caller supplies its own — and the
//! example demonstrates the same contract.
//!
//! A copy of DejaVuSans is checked in at the workspace root for
//! convenience. Run from that directory with:
//!
//! ```text
//! cargo run -p sl-glw --example render_sample -- DejaVuSans.ttf
//! ```

use image::GenericImage;
use sl_glw::{GlwEvent, GlwStyle, MapLikeGlwExt as _};
use sl_map_apis::coverage::PlacementSlot;
use sl_map_apis::map_tiles::MapLike;
use sl_types::map::{GridCoordinates, GridRectangle, GridRectangleLike, ZoomLevel};

/// In-memory `MapLike` impl with no tile-fetching machinery.
struct TestMap {
    /// Pixel image being drawn on.
    image: image::DynamicImage,
    /// Grid rectangle the image represents.
    grid_rectangle: GridRectangle,
    /// Zoom level for pixel↔grid math.
    zoom_level: ZoomLevel,
}

impl GridRectangleLike for TestMap {
    fn grid_rectangle(&self) -> GridRectangle {
        self.grid_rectangle.clone()
    }
}

impl image::GenericImageView for TestMap {
    type Pixel = image::Rgba<u8>;
    fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.image.get_pixel(x, y)
    }
}

impl GenericImage for TestMap {
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        #[expect(deprecated, reason = "delegates to deprecated DynamicImage method")]
        self.image.get_pixel_mut(x, y)
    }
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image.put_pixel(x, y, pixel);
    }
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        #[expect(deprecated, reason = "delegates to deprecated DynamicImage method")]
        self.image.blend_pixel(x, y, pixel);
    }
}

impl MapLike for TestMap {
    fn zoom_level(&self) -> ZoomLevel {
        self.zoom_level
    }
    fn image(&self) -> &image::DynamicImage {
        &self.image
    }
    fn image_mut(&mut self) -> &mut image::DynamicImage {
        &mut self.image
    }
}

/// The exact JSON from `TODO.md` (Lalia's documented sample event).
const SAMPLE_JSON: &str = r#"{
    "eventId": 6910,
    "eventName": "test cruise",
    "eventKey": "key cruise",
    "eventNum": 3381,
    "directorName": "LaliaCasau Resident",
    "directorKey": "b609826a-b167-41e0-8e67-9fc0e78b97a1",
    "sailMode": 2,
    "extra1": "",
    "extra2": "",
    "base": {
        "wind": { "dir": 0, "speed": 17, "gusts": 8, "shifts": 5, "period": 90 },
        "waves": {
            "height": 1.5, "speed": 3, "length": 35,
            "heightVar": 5, "lengthVar": 5,
            "effects": { "speed": 1, "steer": 1 }
        },
        "currents": { "speed": 0, "dir": 180, "waterDepth": 0 }
    },
    "areas": {
        "area1": {
            "coordSW": { "x": 1133, "y": 1048 },
            "coordNE": { "x": 1135, "y": 1049 },
            "margin": 25, "overlap": 0,
            "wind": { "dir": 270, "speed": 12 },
            "currents": { "speed": 1, "dir": 225, "waterDepth": 8 }
        },
        "area2": {
            "coordSW": { "x": 1133, "y": 1050 },
            "coordNE": { "x": 1134, "y": 1053 },
            "margin": 0, "overlap": 0,
            "wind": { "dir": 45, "speed": 14 },
            "currents": { "speed": 1, "dir": 250 }
        }
    },
    "circles": {
        "circle1": {
            "centerSim": { "x": 1136, "y": 1051 },
            "centerPoint": { "x": 90, "y": 175 },
            "radius": 127, "margin": 25, "overlap": 0,
            "wind": { "dir": 135, "speed": 15 },
            "waves": {
                "height": 2, "speed": 3, "length": 35,
                "heightVar": 5, "lengthVar": 5,
                "effects": { "speed": 1, "steer": 1 }
            },
            "currents": { "speed": 0.1, "dir": 225, "waterDepth": 6 }
        },
        "circle2": {
            "centerSim": { "x": 1139, "y": 1051 },
            "centerPoint": { "x": 12, "y": 159 },
            "radius": 261, "margin": 25, "overlap": 0,
            "wind": { "dir": 90, "speed": 18 },
            "currents": { "speed": 2, "dir": 90, "waterDepth": 6 }
        }
    }
}"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Skip argv[0] (binary name) and take the next argument as the
    // font path. Fail with a clear usage message if missing so the
    // contract — "the library doesn't supply a font; you must" — is
    // obvious to anyone running the example.
    let font_path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| -> Box<dyn std::error::Error> {
            "usage: render_sample <path-to-ttf>\n\
         Supply a TrueType font; a copy of DejaVuSans is checked in at \
         the workspace root."
                .into()
        })?;

    let event: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;

    // 11 × 11 sims at zoom 2 → 11 × 128 = 1408 px on each side. That
    // gives the overlay primitives enough room to be readable.
    let grid = GridRectangle::new(
        GridCoordinates::new(1130, 1045),
        GridCoordinates::new(1140, 1055),
    );
    let zoom = ZoomLevel::try_new(2)?;
    let image_size: u32 = u32::from(grid.size_x()) * u32::from(zoom.pixels_per_region());

    // Mid-blue background so the white arrows / green outlines stand
    // out without needing real map tiles for the demo.
    let background = image::Rgba([28u8, 64, 100, 255]);
    let mut map = TestMap {
        image: image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
            image_size, image_size, background,
        )),
        grid_rectangle: grid,
        zoom_level: zoom,
    };

    // Enable the dashed margin band and the corner legend so the
    // sample exercises the override-boundary and legend rendering.
    let style = GlwStyle {
        draw_margin_band: true,
        legend_position: Some(PlacementSlot::TopLeft),
        ..GlwStyle::default()
    };

    // The caller supplies the font; the library never does. Read it
    // from the supplied path and pass it into the `_with_font` API.
    let bytes = fs_err::read(&font_path)?;
    let font = ab_glyph::FontVec::try_from_vec(bytes)?;
    map.draw_glw_event_with_font(&event, &style, &font);

    let out = std::path::Path::new("target/sl-glw-sample.png");
    map.image().save(out)?;
    // Quiet exit: tracing isn't configured for this binary and the
    // workspace's strict clippy bans `println!`. The example is run
    // for the side effect (the saved PNG).
    Ok(())
}
