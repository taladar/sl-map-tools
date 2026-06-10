//! Internal geometry helpers used by the render layer.
//!
//! Direction conventions: GLW expresses wind/current direction as a
//! compass bearing (0 = North, increasing clockwise) representing the
//! direction the wind is blowing **from**. To draw an arrow we need
//! the "blowing toward" unit vector in **image space**, where the y
//! axis points downward. The conversion is centralised in
//! [`blowing_toward_unit_vec`].

use sl_map_apis::map_tiles::MapLike;
use sl_types::map::{GridCoordinates, RegionCoordinates};

use crate::types::WindDirection;

/// Image-space unit vector pointing in the direction the wind is
/// *blowing toward*, given a GLW "blowing from" `WindDirection`.
///
/// Returned as `(dx, dy)` with `dy` increasing downward (image
/// convention).
pub(crate) fn blowing_toward_unit_vec(dir: WindDirection) -> (f32, f32) {
    let toward_deg = (u32::from(dir.degrees()) + 180) % 360;
    let theta = (toward_deg as f32).to_radians();
    (theta.sin(), -theta.cos())
}

/// Compute the pixel coordinates of a circle's centre.
pub(crate) fn circle_center_pixel<M: MapLike + ?Sized>(
    map: &M,
    center_sim: &GridCoordinates,
    center_point: &RegionCoordinates,
) -> Option<(f32, f32)> {
    let (x, y) = map.pixel_coordinates_for_coordinates(center_sim, center_point)?;
    Some((x as f32, y as f32))
}

/// Draw a dashed straight-line segment from `a` to `b`.
///
/// `dash` is the length in pixels of each "on" segment, `gap` the
/// length of each "off" interval. The first dash starts at `a`. If
/// `dash + gap <= 0` the call is a no-op. The final dash is clipped at
/// `b` so the line doesn't overshoot the requested endpoint.
pub(crate) fn dashed_line<M: MapLike + ?Sized>(
    map: &mut M,
    a: (f32, f32),
    b: (f32, f32),
    dash: f32,
    gap: f32,
    color: image::Rgba<u8>,
) {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length = (dx * dx + dy * dy).sqrt();
    let period = dash + gap;
    if length < 0.5 || period <= 0.0 || dash <= 0.0 {
        return;
    }
    let ux = dx / length;
    let uy = dy / length;
    let mut t = 0.0_f32;
    while t < length {
        let t_end = (t + dash).min(length);
        let p0 = (a.0 + ux * t, a.1 + uy * t);
        let p1 = (a.0 + ux * t_end, a.1 + uy * t_end);
        imageproc::drawing::draw_line_segment_mut(map.image_mut(), p0, p1, color);
        t += period;
    }
}

/// Draw a dashed axis-aligned rectangle by stroking each of its four
/// sides with [`dashed_line`].
///
/// `top_left` and `bottom_right` are in image-space pixel coordinates
/// (`+y` downward).
pub(crate) fn dashed_rect<M: MapLike + ?Sized>(
    map: &mut M,
    top_left: (f32, f32),
    bottom_right: (f32, f32),
    dash: f32,
    gap: f32,
    color: image::Rgba<u8>,
) {
    let (x0, y0) = top_left;
    let (x1, y1) = bottom_right;
    dashed_line(map, (x0, y0), (x1, y0), dash, gap, color);
    dashed_line(map, (x1, y0), (x1, y1), dash, gap, color);
    dashed_line(map, (x1, y1), (x0, y1), dash, gap, color);
    dashed_line(map, (x0, y1), (x0, y0), dash, gap, color);
}

/// Draw a dashed circle by sampling angles around `center` and
/// stroking short chord segments for each "on" arc.
///
/// Dash and gap are arc-length pixels (so dashes stay visually
/// uniform regardless of radius). The number of (dash + gap) periods
/// is rounded to the nearest integer so dashes wrap evenly around the
/// full circumference.
pub(crate) fn dashed_circle<M: MapLike + ?Sized>(
    map: &mut M,
    center: (f32, f32),
    radius: f32,
    dash: f32,
    gap: f32,
    color: image::Rgba<u8>,
) {
    let period = dash + gap;
    if radius < 1.0 || period <= 0.0 || dash <= 0.0 {
        return;
    }
    let circumference = 2.0 * std::f32::consts::PI * radius;
    let n = (circumference / period).round().max(1.0) as u32;
    let period_angle = 2.0 * std::f32::consts::PI / n as f32;
    let dash_angle = period_angle * (dash / period);
    // Sub-divide each dash into enough chord segments that the chord
    // stays close to the underlying arc visually. Roughly one chord
    // every two pixels of arc.
    let chords_per_dash = ((dash * 0.5).ceil() as u32).max(2);
    for i in 0..n {
        let start_angle = i as f32 * period_angle;
        let mut prev: Option<(f32, f32)> = None;
        for j in 0..=chords_per_dash {
            let t = j as f32 / chords_per_dash as f32;
            let a = start_angle + t * dash_angle;
            let p = (center.0 + radius * a.cos(), center.1 + radius * a.sin());
            if let Some(prev_p) = prev {
                imageproc::drawing::draw_line_segment_mut(map.image_mut(), prev_p, p, color);
            }
            prev = Some(p);
        }
    }
}

/// 8-point compass label for a `WindDirection`.
///
/// Returns one of `"N"`, `"NE"`, `"E"`, `"SE"`, `"S"`, `"SW"`, `"W"`,
/// `"NW"`. Picks the nearest cardinal/intercardinal by rounding to the
/// nearest 45° sector. Used in `tracing::debug!` output during render
/// and by future text labels.
pub(crate) fn degrees_to_compass8(dir: WindDirection) -> &'static str {
    // bucket = (d + 22) % 360 / 45 is in 0..=7 for d in 0..360,
    // so the `_` arm is unreachable for any valid input. It's folded
    // into the `N` branch via `0 | _` to keep clippy's
    // `match_same_arms` happy.
    let bucket = ((u32::from(dir.degrees()) + 22) % 360) / 45;
    match bucket {
        1 => "NE",
        2 => "E",
        3 => "SE",
        4 => "S",
        5 => "SW",
        6 => "W",
        7 => "NW",
        _ => "N",
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn blowing_toward_cardinal_directions() -> Result<(), Box<dyn std::error::Error>> {
        // direction is "blowing from"; arrow points in the opposite
        // direction in image space (y increases downward).
        let cases = [
            (0_u16, (0.0_f32, 1.0_f32)),
            (90, (-1.0, 0.0)),
            (180, (0.0, -1.0)),
            (270, (1.0, 0.0)),
        ];
        for (deg, expected) in cases {
            let dir = WindDirection::try_new(deg)?;
            let (dx, dy) = blowing_toward_unit_vec(dir);
            assert!((dx - expected.0).abs() < 1e-5, "dx for {deg}");
            assert!((dy - expected.1).abs() < 1e-5, "dy for {deg}");
        }
        Ok(())
    }

    #[test]
    fn compass8_buckets() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(degrees_to_compass8(WindDirection::try_new(0)?), "N");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(22)?), "N");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(23)?), "NE");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(45)?), "NE");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(90)?), "E");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(135)?), "SE");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(180)?), "S");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(225)?), "SW");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(270)?), "W");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(315)?), "NW");
        assert_eq!(degrees_to_compass8(WindDirection::try_new(359)?), "N");
        Ok(())
    }
}
