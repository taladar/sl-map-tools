//! Arc-length geometry helpers for rasterising a route spline.
//!
//! The route is a Catmull-Rom spline, and its arc length per unit of spline
//! parameter is not constant: it stretches where waypoints are far apart and
//! compresses on tight bends. Stepping the parameter uniformly therefore
//! bunches the dots up on bends and spreads them out on long straight legs,
//! which is the unevenness this module exists to remove.
//!
//! [`RouteArcTable`] densely samples the curve once, accumulates the arc
//! length along it, and then answers "where is the point this many pixels
//! along the route" and "which way is right of travel there". Dashes are
//! placed in those pixel units, so they come out evenly spaced no matter what
//! the parameterisation does.
//!
//! Direction convention: pixel coordinates have `+y` pointing down, so the
//! normal pointing right of a traveller heading along `(dx, dy)` is
//! `(-dy, dx)` — the same perpendicular
//! [`crate::map_tiles::MapLike::draw_line`] uses. Heading east, `(1, 0)`,
//! that gives `(0, 1)`, i.e. south, which is indeed the traveller's right.

/// fewest dense samples taken along one leg, so even a very short leg still
/// has enough points for a usable arc-length table
const MIN_SAMPLES_PER_LEG: u32 = 16;

/// most dense samples taken along one leg, so a pathologically long leg cannot
/// blow up the table
const MAX_SAMPLES_PER_LEG: u32 = 4096;

/// shortest tangent, in pixels, still considered a usable direction; below
/// this the central difference is treated as degenerate and widened
const MIN_TANGENT_LENGTH: f32 = 1e-4;

/// the distance in pixels between two points
fn distance(from: (f32, f32), to: (f32, f32)) -> f32 {
    ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt()
}

/// move `point` sideways by `offset` pixels along the unit normal `normal`
pub(crate) fn offset_point(point: (f32, f32), normal: (f32, f32), offset: f32) -> (f32, f32) {
    (point.0 + normal.0 * offset, point.1 + normal.1 * offset)
}

/// A densely sampled route curve with its cumulative arc length, so positions
/// along the route can be addressed in pixels rather than in spline parameter.
///
/// Built once for the whole route rather than per leg, so the finite
/// differences used for the normals cross leg boundaries and the offset curve
/// stays continuous where one leg meets the next.
#[derive(Debug, Clone)]
pub(crate) struct RouteArcTable {
    /// the densely sampled points along the curve, in order
    points: Vec<(f32, f32)>,
    /// `arc[k]` is the cumulative arc length in pixels up to `points[k]`, so
    /// `arc[0]` is 0 and the last entry is the length of the whole route
    arc: Vec<f32>,
    /// `waypoint_arc[i]` is the arc length at waypoint `i`
    waypoint_arc: Vec<f32>,
}

impl RouteArcTable {
    /// Densely sample the route and accumulate its arc length.
    ///
    /// `sample` evaluates the spline at a parameter in `[0, 1]`. Waypoint `i`
    /// is taken to sit at parameter `i / (waypoint_count - 1)`, which is where
    /// `uniform_cubic_splines` puts it for a Catmull-Rom basis given the
    /// waypoints plus two phantom control points.
    ///
    /// # Errors
    ///
    /// propagates any error from `sample`
    pub(crate) fn build<F, E>(waypoint_count: usize, sample: F) -> Result<Self, E>
    where
        F: Fn(f32) -> Result<(f32, f32), E>,
    {
        if waypoint_count < 2 {
            return Ok(Self {
                points: Vec::new(),
                arc: Vec::new(),
                waypoint_arc: Vec::new(),
            });
        }
        let segments = waypoint_count - 1;
        #[expect(
            clippy::cast_precision_loss,
            reason = "if our waypoint counts get anywhere near 2^23 routes probably will not be finished anyway"
        )]
        let denominator = segments as f32;
        let start = sample(0f32)?;
        let mut points = vec![start];
        let mut arc = vec![0f32];
        let mut waypoint_arc = vec![0f32];
        for leg in 0..segments {
            #[expect(
                clippy::cast_precision_loss,
                reason = "if our waypoint counts get anywhere near 2^23 routes probably will not be finished anyway"
            )]
            let leg_start_parameter = leg as f32 / denominator;
            #[expect(
                clippy::cast_precision_loss,
                reason = "if our waypoint counts get anywhere near 2^23 routes probably will not be finished anyway"
            )]
            let leg_end_parameter = (leg + 1) as f32 / denominator;
            let chord = distance(sample(leg_start_parameter)?, sample(leg_end_parameter)?);
            // roughly one dense sample per pixel of chord, so the polyline
            // tracks the curve closely without exploding on long legs
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the value is clamped into [MIN_SAMPLES_PER_LEG, MAX_SAMPLES_PER_LEG] before the cast"
            )]
            let samples = if chord.is_finite() {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "the sample bounds are small integers well within f32"
                )]
                let clamped = chord
                    .ceil()
                    .clamp(MIN_SAMPLES_PER_LEG as f32, MAX_SAMPLES_PER_LEG as f32);
                clamped as u32
            } else {
                MIN_SAMPLES_PER_LEG
            };
            for step in 1..=samples {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "the sample counts are bounded by MAX_SAMPLES_PER_LEG"
                )]
                let within_leg =
                    f32::from(u16::try_from(step).unwrap_or(u16::MAX)) / samples as f32;
                let parameter = leg_end_parameter
                    .mul_add(within_leg, leg_start_parameter * (1f32 - within_leg));
                let point = sample(parameter)?;
                let previous = points.last().copied().unwrap_or(point);
                let previous_arc = arc.last().copied().unwrap_or(0f32);
                points.push(point);
                arc.push(previous_arc + distance(previous, point));
            }
            waypoint_arc.push(arc.last().copied().unwrap_or(0f32));
        }
        Ok(Self {
            points,
            arc,
            waypoint_arc,
        })
    }

    /// the arc length of the whole route in pixels
    pub(crate) fn total_length(&self) -> f32 {
        self.arc.last().copied().unwrap_or(0f32)
    }

    /// the arc length range `(start, end)` of the leg leading away from
    /// waypoint `leg`, or `None` when there is no such leg
    pub(crate) fn leg_arc_range(&self, leg: usize) -> Option<(f32, f32)> {
        let start = self.waypoint_arc.get(leg).copied()?;
        let end = self.waypoint_arc.get(leg + 1).copied()?;
        Some((start, end))
    }

    /// the point `arc_length` pixels along the route, clamped to its ends
    pub(crate) fn point_at_arc(&self, arc_length: f32) -> (f32, f32) {
        let fallback = self.points.last().copied().unwrap_or((0f32, 0f32));
        let arc_length = if arc_length.is_finite() {
            arc_length.clamp(0f32, self.total_length())
        } else {
            return fallback;
        };
        // the first index whose cumulative length is still at or below the
        // wanted one, so the wanted point lies in the span starting there
        let index = self
            .arc
            .partition_point(|entry| *entry <= arc_length)
            .saturating_sub(1);
        let (Some(span_start_arc), Some(span_end_arc), Some(span_start), Some(span_end)) = (
            self.arc.get(index).copied(),
            self.arc.get(index + 1).copied(),
            self.points.get(index).copied(),
            self.points.get(index + 1).copied(),
        ) else {
            return fallback;
        };
        let span = span_end_arc - span_start_arc;
        let fraction = if span.abs() < f32::EPSILON {
            0f32
        } else {
            (arc_length - span_start_arc) / span
        };
        (
            (span_end.0 - span_start.0).mul_add(fraction, span_start.0),
            (span_end.1 - span_start.1).mul_add(fraction, span_start.1),
        )
    }

    /// The unit normal pointing right of the direction of travel `arc_length`
    /// pixels along the route, or `None` where no usable direction exists.
    ///
    /// The direction comes from a central difference **in arc length**, which
    /// is what makes it independent of the uneven spline parameterisation.
    /// `spacing_hint` is the half width of that difference in pixels; it is
    /// widened on degenerate input (identical consecutive waypoints, a cusp) and
    /// only gives up once even the whole route yields no direction.
    pub(crate) fn unit_normal_at_arc(
        &self,
        arc_length: f32,
        spacing_hint: f32,
    ) -> Option<(f32, f32)> {
        let total = self.total_length();
        if total <= 0f32 {
            return None;
        }
        let hint = if spacing_hint.is_finite() {
            spacing_hint.max(1f32)
        } else {
            1f32
        };
        for spacing in [hint, hint * 4f32, hint * 16f32, total] {
            let before = self.point_at_arc((arc_length - spacing).max(0f32));
            let after = self.point_at_arc((arc_length + spacing).min(total));
            let delta = (after.0 - before.0, after.1 - before.1);
            let magnitude = (delta.0.powi(2) + delta.1.powi(2)).sqrt();
            if magnitude.is_finite() && magnitude > MIN_TANGENT_LENGTH {
                // +y is down, so right of travel is (-dy, dx)
                return Some((-delta.1 / magnitude, delta.0 / magnitude));
            }
        }
        None
    }
}

/// The `(start, end)` arc-length offsets of each drawn run within a leg of
/// `length` pixels.
///
/// A zero (or negative) `gap` means a solid line: one run covering the whole
/// leg. Otherwise the period is rounded to a whole number of repeats over the
/// leg and the dash length scaled to match, keeping the duty cycle, so a run
/// starts exactly at the leg's first waypoint and one ends exactly at its
/// last. That is what puts a dot on every waypoint and lets each leg be
/// rasterised independently in its own style.
pub(crate) fn dashes_for_leg(length: f32, dash: f32, gap: f32) -> Vec<(f32, f32)> {
    if !length.is_finite() || length <= 0f32 || !dash.is_finite() || !gap.is_finite() {
        return Vec::new();
    }
    if gap <= 0f32 {
        return vec![(0f32, length)];
    }
    if dash <= 0f32 {
        return Vec::new();
    }
    let period = dash + gap;
    if period <= 0f32 {
        return vec![(0f32, length)];
    }
    let repeats = (length / period).round().max(1f32);
    let adjusted_period = length / repeats;
    let adjusted_dash = adjusted_period * (dash / period);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "repeats is a rounded positive count bounded by the leg length in pixels"
    )]
    let repeats_count = repeats.min(f32::from(u16::MAX)) as u32;
    (0..repeats_count)
        .map(|index| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "the repeat count is bounded by u16::MAX"
            )]
            let start = index as f32 * adjusted_period;
            (start, start + adjusted_dash)
        })
        .collect()
}

#[cfg(test)]
mod test {
    #![expect(
        clippy::float_cmp,
        reason = "the arc-length arithmetic under test is exact for the straight-line fixtures used here"
    )]

    use super::{RouteArcTable, dashes_for_leg, distance, offset_point};
    use pretty_assertions::assert_eq;

    /// the error type of the fixture samplers, which never fail
    type Never = std::convert::Infallible;

    /// a straight horizontal route: parameter 0 at x = 0, parameter 1 at x = 200
    fn straight_sampler(parameter: f32) -> Result<(f32, f32), Never> {
        Ok((parameter * 200f32, 50f32))
    }

    /// a quarter circle of radius 100 centred on the origin, which has a
    /// strongly non-uniform relationship between parameter and arc length once
    /// sampled the way the route code samples a spline
    fn curved_sampler(parameter: f32) -> Result<(f32, f32), Never> {
        // squaring the parameter makes equal parameter steps cover very
        // different arc lengths, which is exactly the spline behaviour that
        // made the old renderer space its dots unevenly
        let angle = parameter * parameter * std::f32::consts::FRAC_PI_2;
        Ok((100f32 * angle.cos(), 100f32 * angle.sin()))
    }

    /// The table's total length must match a known straight-line length, and
    /// the cumulative arc must never go backwards.
    #[test]
    fn test_arc_table_is_monotonic_and_matches_a_straight_line()
    -> Result<(), Box<dyn std::error::Error>> {
        let table = RouteArcTable::build(3, straight_sampler)?;
        let total = table.total_length();
        assert!(
            (total - 200f32).abs() < 2f32,
            "a 200 px straight route should measure ~200 px, got {total}"
        );
        let mut previous = 0f32;
        for step in 0u16..=100u16 {
            let arc = f32::from(step) * total / 100f32;
            let point = table.point_at_arc(arc);
            assert!(point.0 >= previous - 1f32, "x must not go backwards");
            previous = point.0;
        }
        Ok(())
    }

    /// The per-waypoint arc lengths must land on the waypoints themselves.
    #[test]
    fn test_waypoint_arc_lengths_land_on_the_waypoints() -> Result<(), Box<dyn std::error::Error>> {
        let table = RouteArcTable::build(3, straight_sampler)?;
        let (first_start, first_end) = table
            .leg_arc_range(0)
            .ok_or("a three-waypoint route has a first leg")?;
        assert_eq!(first_start, 0f32, "the route starts at arc length 0");
        let midpoint = table.point_at_arc(first_end);
        assert!(
            (midpoint.0 - 100f32).abs() < 2f32,
            "the middle waypoint of a 200 px route sits at x ~ 100, got {midpoint:?}"
        );
        assert!(
            table.leg_arc_range(2).is_none(),
            "a three-waypoint route has only two legs"
        );
        Ok(())
    }

    /// The regression test for the uneven dot spacing: on a curve whose
    /// parameter maps very unevenly onto arc length, dashes placed by arc
    /// length must still come out evenly spaced.
    #[test]
    fn test_dash_positions_are_evenly_spaced_in_arc_length()
    -> Result<(), Box<dyn std::error::Error>> {
        let table = RouteArcTable::build(2, curved_sampler)?;
        let (start, end) = table
            .leg_arc_range(0)
            .ok_or("a two-waypoint route has one leg")?;
        let dashes = dashes_for_leg(end - start, 3f32, 3f32);
        assert!(
            dashes.len() > 5,
            "the fixture should produce several dashes"
        );
        let centers: Vec<(f32, f32)> = dashes
            .iter()
            .map(|(dash_start, dash_end)| {
                table.point_at_arc(start + 0.5f32 * (dash_start + dash_end))
            })
            .collect();
        let spacings: Vec<f32> = centers
            .windows(2)
            .filter_map(|pair| match pair {
                [first, second] => Some(distance(*first, *second)),
                _ => None,
            })
            .collect();
        let max = spacings.iter().copied().fold(f32::MIN, f32::max);
        let min = spacings.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            max / min < 1.05f32,
            "dash spacing should be uniform in pixels, got min {min} max {max} (ratio {})",
            max / min
        );
        Ok(())
    }

    /// Dashes must wrap evenly over a leg: one starts at the leg start and the
    /// last one ends at the leg end, so waypoints always carry a dot.
    #[test]
    fn test_dashes_wrap_evenly_over_a_leg() -> Result<(), Box<dyn std::error::Error>> {
        let dashes = dashes_for_leg(100f32, 3f32, 3f32);
        let (first_start, _) = dashes
            .first()
            .copied()
            .ok_or("a 100 px leg must produce dashes")?;
        assert_eq!(first_start, 0f32, "a dash starts exactly at the leg start");
        let (last_start, _) = dashes
            .last()
            .copied()
            .ok_or("a 100 px leg must produce dashes")?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "the dash count for a 100 px leg is tiny"
        )]
        let period = 100f32 / dashes.len() as f32;
        assert!(
            (last_start + period - 100f32).abs() < 1e-3,
            "the last period must end exactly at the leg end"
        );
        Ok(())
    }

    /// A zero gap means solid: one run covering the whole leg.
    #[test]
    fn test_zero_gap_is_a_solid_run() {
        assert_eq!(dashes_for_leg(100f32, 3f32, 0f32), vec![(0f32, 100f32)]);
    }

    /// Degenerate dash parameters must produce nothing rather than panicking or
    /// looping forever.
    #[test]
    fn test_degenerate_dash_parameters_draw_nothing() {
        assert!(dashes_for_leg(0f32, 3f32, 3f32).is_empty());
        assert!(dashes_for_leg(-5f32, 3f32, 3f32).is_empty());
        assert!(dashes_for_leg(100f32, 0f32, 3f32).is_empty());
        assert!(dashes_for_leg(f32::NAN, 3f32, 3f32).is_empty());
        assert!(dashes_for_leg(100f32, f32::NAN, 3f32).is_empty());
    }

    /// Queries outside the route must clamp to its ends rather than panic.
    #[test]
    fn test_point_at_arc_clamps_out_of_range_queries() -> Result<(), Box<dyn std::error::Error>> {
        let table = RouteArcTable::build(2, straight_sampler)?;
        let total = table.total_length();
        let before = table.point_at_arc(-10f32);
        assert!(
            (before.0 - 0f32).abs() < 1f32,
            "before the start clamps to it"
        );
        let after = table.point_at_arc(total + 10f32);
        assert!(
            (after.0 - 200f32).abs() < 1f32,
            "past the end clamps to it, got {after:?}"
        );
        let nan = table.point_at_arc(f32::NAN);
        assert!(
            nan.0.is_finite() && nan.1.is_finite(),
            "a NaN query stays finite"
        );
        Ok(())
    }

    /// The normal must be a unit vector perpendicular to travel and point to
    /// the traveller's right, which is south for an eastbound route.
    #[test]
    fn test_normal_is_perpendicular_and_right_handed() -> Result<(), Box<dyn std::error::Error>> {
        let table = RouteArcTable::build(2, straight_sampler)?;
        let normal = table
            .unit_normal_at_arc(100f32, 3f32)
            .ok_or("a straight route has a well defined normal")?;
        assert!(
            (normal.0).abs() < 1e-3,
            "an eastbound route's normal has no x component, got {normal:?}"
        );
        assert!(
            (normal.1 - 1f32).abs() < 1e-3,
            "right of eastbound travel is south (+y in image space), got {normal:?}"
        );
        let magnitude = (normal.0.powi(2) + normal.1.powi(2)).sqrt();
        assert!(
            (magnitude - 1f32).abs() < 1e-3,
            "the normal must be a unit vector"
        );
        Ok(())
    }

    /// A route that never moves has no direction; the caller must get `None`
    /// rather than a NaN normal.
    #[test]
    fn test_degenerate_route_yields_no_normal() -> Result<(), Box<dyn std::error::Error>> {
        let table = RouteArcTable::build(2, |_| Ok::<_, Never>((10f32, 10f32)))?;
        assert!(
            table.unit_normal_at_arc(0f32, 3f32).is_none(),
            "a zero length route has no direction"
        );
        let point = table.point_at_arc(0f32);
        assert!(
            point.0.is_finite() && point.1.is_finite(),
            "its points must still be finite"
        );
        Ok(())
    }

    /// Offsetting moves the point along the normal by exactly the offset.
    #[test]
    fn test_offset_point_moves_along_the_normal() {
        assert_eq!(
            offset_point((10f32, 20f32), (0f32, 1f32), 5f32),
            (10f32, 25f32)
        );
        assert_eq!(
            offset_point((10f32, 20f32), (0f32, 1f32), -5f32),
            (10f32, 15f32)
        );
    }

    /// A route with fewer than two waypoints has no geometry at all.
    #[test]
    fn test_short_routes_build_an_empty_table() -> Result<(), Box<dyn std::error::Error>> {
        let table = RouteArcTable::build(1, straight_sampler)?;
        assert_eq!(table.total_length(), 0f32);
        assert!(table.leg_arc_range(0).is_none());
        Ok(())
    }
}
