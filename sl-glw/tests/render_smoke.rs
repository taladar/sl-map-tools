//! End-to-end smoke test: parse the documented sample JSON into a
//! `GlwEvent`, draw it onto an in-memory map, and confirm the render
//! actually wrote pixels.
//!
//! Uses a minimal local `MapLike` implementation so the test doesn't
//! depend on the real `sl_map_apis::Map` (which needs a tile cache and
//! HTTP). This keeps the smoke test hermetic.

#![expect(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently the test module"
)]

use image::GenericImageView as _;
use pretty_assertions::assert_eq;
use sl_glw::{GlwEvent, GlwStyle, MapLikeGlwExt as _};
use sl_map_apis::map_tiles::MapLike;
use sl_types::map::{GridCoordinates, GridRectangle, GridRectangleLike, ZoomLevel};

/// Minimal `MapLike` implementation for tests. Wraps a `DynamicImage`
/// of known size and a fixed grid rectangle. Pixel↔grid conversions go
/// through the default trait methods.
struct TestMap {
    /// In-memory image backing the map.
    image: image::DynamicImage,
    /// Grid rectangle the image represents.
    grid_rectangle: GridRectangle,
    /// Zoom level used by the default pixel-math methods.
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

impl image::GenericImage for TestMap {
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        #[expect(
            deprecated,
            reason = "passes through to the equally deprecated DynamicImage method"
        )]
        self.image.get_pixel_mut(x, y)
    }
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image.put_pixel(x, y, pixel);
    }
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        #[expect(
            deprecated,
            reason = "passes through to the equally deprecated DynamicImage method"
        )]
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

/// Minimal but complete sample JSON exercising areas and circles. Same
/// shape as the doc sample in the repo `TODO.md`.
const SAMPLE_JSON: &str = r#"{
    "eventId": 6910,
    "eventName": "test cruise",
    "eventKey": "key cruise",
    "directorName": "LaliaCasau Resident",
    "directorKey": "b609826a-b167-41e0-8e67-9fc0e78b97a1",
    "base": {
        "wind": { "dir": 175, "speed": 17, "gusts": 8, "shifts": 5, "period": 90 },
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
            "currents": { "speed": 1, "dir": 225, "waterDepth": 8 }
        }
    },
    "circles": {
        "circle1": {
            "centerSim": { "x": 1136, "y": 1051 },
            "centerPoint": { "x": 90, "y": 175 },
            "radius": 127, "margin": 25, "overlap": 0,
            "wind": { "speed": 15 },
            "currents": { "speed": 0.1, "dir": 225, "waterDepth": 6 }
        }
    }
}"#;

/// Render the sample event onto a fresh black image and assert the
/// draw actually touched some pixels. The exact output is not asserted
/// — we just confirm the pipeline doesn't panic and does produce
/// visible output.
#[test]
fn draw_glw_event_modifies_image() -> Result<(), Box<dyn std::error::Error>> {
    let event: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;

    let grid = GridRectangle::new(
        GridCoordinates::new(1130, 1045),
        GridCoordinates::new(1140, 1055),
    );
    // 11 sims wide × 11 tall. At zoom level 4 each sim is 32 px, so the
    // projection covers 352×352 px — pick that as the image size so
    // grid math stays in-bounds.
    let zoom = ZoomLevel::try_new(4)?;
    let image_size: u32 = grid.size_x() * u32::from(zoom.pixels_per_region());
    let mut map = TestMap {
        image: image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
            image_size,
            image_size,
            image::Rgba([0u8, 0, 0, 255]),
        )),
        grid_rectangle: grid,
        zoom_level: zoom,
    };

    let (pre_w, pre_h) = MapLike::image(&map).dimensions();
    assert_eq!((pre_w, pre_h), (image_size, image_size));
    let pixel_before = image::Rgba([0u8, 0, 0, 255]);

    map.draw_glw_event(&event, &GlwStyle::default());

    // At least *some* pixel in the image must have changed colour.
    let mut changed = false;
    'outer: for x in 0..image_size {
        for y in 0..image_size {
            if image::GenericImageView::get_pixel(&map, x, y) != pixel_before {
                changed = true;
                break 'outer;
            }
        }
    }
    assert!(
        changed,
        "draw_glw_event should write at least one non-black pixel"
    );
    Ok(())
}
