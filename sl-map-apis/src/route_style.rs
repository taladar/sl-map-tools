//! Visual style for a route drawn from a USB notecard.
//!
//! A route's style is a list of [`RouteSectionStyle`] entries in waypoint
//! order. Each entry takes effect at its `from_waypoint` and stays in effect
//! until the next entry overrides it, so the entry at waypoint 0 is simply the
//! route-wide style and there is no separate "global" concept: one list, one
//! cascade rule.
//!
//! Every field of an entry but the waypoint index is optional, and `None`
//! means "keep whatever is currently in effect". That is what makes a change
//! like "from waypoint 12 onwards draw it yellow" keep the thickness an
//! earlier entry set, rather than snapping back to the defaults.
//!
//! Front-ends are free to keep presenting a route-wide style and a list of
//! per-section changes as two separate things (the CLI flags and the web form
//! do); they just build the waypoint-0 entry and the later entries
//! respectively.
//!
//! The defaults reproduce the look the renderer had before this module
//! existed: a red, 3 pixel wide dotted line with a 3 pixel gap, no lateral
//! offset, and one arrowhead on every waypoint but the first.

/// colour used when no entry sets one: the historical route red
const DEFAULT_COLOR: image::Rgba<u8> = image::Rgba([255, 0, 0, 255]);

/// line thickness in pixels used when no entry sets one
const DEFAULT_THICKNESS_PIXELS: f32 = 3.0;

/// smallest accepted line thickness in pixels
const MIN_THICKNESS_PIXELS: f32 = 1.0;

/// largest accepted line thickness in pixels
const MAX_THICKNESS_PIXELS: f32 = 256.0;

/// largest accepted dash or gap length in pixels
const MAX_DASH_PIXELS: f32 = 4096.0;

/// largest accepted absolute lateral offset in pixels
const MAX_OFFSET_PIXELS: f32 = 256.0;

/// arrow scale used when no entry sets one
const DEFAULT_ARROW_SCALE: f32 = 1.0;

/// largest accepted arrow scale factor
const MAX_ARROW_SCALE: f32 = 64.0;

/// arrow length in pixels per pixel of line thickness, so the historical
/// 15 pixel arrow falls out of the historical 3 pixel thickness
const ARROW_LENGTH_PER_THICKNESS: f32 = 5.0;

/// arrow half width in pixels per pixel of line thickness, so the historical
/// 5 pixel half width falls out of the historical 3 pixel thickness
const ARROW_HALF_WIDTH_PER_THICKNESS: f32 = 5.0 / 3.0;

/// how the route line itself is stroked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RouteLineStyle {
    /// one continuous run with no gaps
    Solid,
    /// long dashes separated by gaps
    Dashed,
    /// short dots one line thickness long, the historical look
    #[default]
    Dotted,
}

impl RouteLineStyle {
    /// the dash and gap lengths in pixels this line style implies at the given
    /// line thickness, used whenever no entry sets them explicitly.
    ///
    /// [`Self::Solid`] returns a zero gap, which the renderer reads as "one
    /// dash covering the whole leg", so its dash length is irrelevant.
    #[must_use]
    pub fn dash_and_gap_pixels(self, thickness_pixels: f32) -> (f32, f32) {
        match self {
            Self::Solid => (thickness_pixels, 0.0),
            Self::Dashed => (4.0 * thickness_pixels, 2.0 * thickness_pixels),
            Self::Dotted => (thickness_pixels, thickness_pixels),
        }
    }
}

/// where direction arrowheads are placed along the route
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteArrowPlacement {
    /// no direction arrows at all
    None,
    /// one arrow at the end of every nth leg, so `1` puts one on every
    /// waypoint but the first
    EveryNthWaypoint(std::num::NonZeroU32),
    /// arrows spaced by a fixed arc length in pixels, with at least one per leg
    EveryPixels(f32),
}

impl Default for RouteArrowPlacement {
    fn default() -> Self {
        Self::EveryNthWaypoint(std::num::NonZeroU32::MIN)
    }
}

/// a style change taking effect at [`Self::from_waypoint`] and cascading
/// forward until the next entry overrides it.
///
/// Every field but the waypoint index is optional; `None` means "keep whatever
/// is currently in effect". An entry at waypoint 0 therefore sets the
/// route-wide style.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RouteSectionStyle {
    /// index of the waypoint this style change takes effect at
    pub from_waypoint: usize,
    /// colour of the line and its arrowheads
    pub color: Option<image::Rgba<u8>>,
    /// line thickness in pixels
    pub thickness_pixels: Option<f32>,
    /// lateral offset in pixels, positive being right of the direction of
    /// travel, which draws the two passes of an out-and-back route as two
    /// separate lines
    pub offset_pixels: Option<f32>,
    /// solid, dashed or dotted
    pub line_style: Option<RouteLineStyle>,
    /// explicit dash length in pixels, overriding what [`Self::line_style`]
    /// would derive
    pub dash_length_pixels: Option<f32>,
    /// explicit gap length in pixels, overriding what [`Self::line_style`]
    /// would derive
    pub gap_length_pixels: Option<f32>,
    /// where arrowheads go
    pub arrow_placement: Option<RouteArrowPlacement>,
    /// multiplier on the thickness-derived arrowhead size
    pub arrow_scale: Option<f32>,
}

impl RouteSectionStyle {
    /// a section changing nothing, starting at the given waypoint
    #[must_use]
    pub fn new(from_waypoint: usize) -> Self {
        Self {
            from_waypoint,
            ..Self::default()
        }
    }
}

/// the style of a whole route: style changes in waypoint order
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RouteStyle {
    /// the style changes, each cascading from its waypoint to the end of the
    /// route. Not required to be sorted; resolution sorts a local copy.
    sections: Vec<RouteSectionStyle>,
}

impl RouteStyle {
    /// a route drawn entirely in the given colour, everything else default
    #[must_use]
    pub fn new(color: image::Rgba<u8>) -> Self {
        Self {
            sections: vec![RouteSectionStyle {
                color: Some(color),
                ..RouteSectionStyle::new(0)
            }],
        }
    }

    /// the style changes, in the order they were added
    #[must_use]
    pub fn sections(&self) -> &[RouteSectionStyle] {
        &self.sections
    }

    /// the entry at waypoint 0 (the route-wide style), inserting an empty one
    /// if there is none yet.
    ///
    /// This is what front-ends that present a route-wide style separately from
    /// per-section changes write their route-wide controls into.
    pub fn base_mut(&mut self) -> &mut RouteSectionStyle {
        if !self
            .sections
            .iter()
            .any(|section| section.from_waypoint == 0)
        {
            self.sections.insert(0, RouteSectionStyle::new(0));
        }
        // the entry was just guaranteed to exist
        let Some(base) = self
            .sections
            .iter_mut()
            .find(|section| section.from_waypoint == 0)
        else {
            unreachable!("a waypoint 0 entry was just inserted if it was missing")
        };
        base
    }

    /// set the route-wide line thickness in pixels
    #[must_use]
    pub fn with_thickness(mut self, thickness_pixels: f32) -> Self {
        self.base_mut().thickness_pixels = Some(thickness_pixels);
        self
    }

    /// set the route-wide lateral offset in pixels, positive being right of
    /// the direction of travel
    #[must_use]
    pub fn with_offset(mut self, offset_pixels: f32) -> Self {
        self.base_mut().offset_pixels = Some(offset_pixels);
        self
    }

    /// set the route-wide line style
    #[must_use]
    pub fn with_line_style(mut self, line_style: RouteLineStyle) -> Self {
        self.base_mut().line_style = Some(line_style);
        self
    }

    /// set the route-wide arrow placement
    #[must_use]
    pub fn with_arrow_placement(mut self, arrow_placement: RouteArrowPlacement) -> Self {
        self.base_mut().arrow_placement = Some(arrow_placement);
        self
    }

    /// add a style change; entries need not be added in waypoint order
    #[must_use]
    pub fn with_section(mut self, section: RouteSectionStyle) -> Self {
        self.sections.push(section);
        self
    }

    /// add a style change; entries need not be added in waypoint order
    pub fn push_section(&mut self, section: RouteSectionStyle) {
        self.sections.push(section);
    }

    /// the fully resolved style in effect at each waypoint of a route with
    /// `waypoint_count` waypoints.
    ///
    /// Resolution starts from the defaults and applies every entry whose
    /// `from_waypoint` is at or before the waypoint in question, in ascending
    /// order, overwriting only the fields that entry sets. Entries with the
    /// same `from_waypoint` apply in the order they were added, and entries
    /// past the end of the route are simply never reached.
    ///
    /// The renderer uses the style at a leg's *starting* waypoint for the whole
    /// leg, so a change at waypoint 12 is visible from waypoint 12 onwards.
    #[must_use]
    pub fn effective_styles_per_waypoint(&self, waypoint_count: usize) -> Vec<ResolvedRouteStyle> {
        let mut sorted: Vec<&RouteSectionStyle> = self.sections.iter().collect();
        // stable, so entries sharing a waypoint keep their declaration order
        sorted.sort_by_key(|section| section.from_waypoint);
        let mut raw = RawRouteStyle::default();
        let mut next = 0usize;
        let mut resolved = Vec::with_capacity(waypoint_count);
        for waypoint in 0..waypoint_count {
            while let Some(section) = sorted.get(next) {
                if section.from_waypoint > waypoint {
                    break;
                }
                raw.apply(section);
                next += 1;
            }
            resolved.push(raw.resolve());
        }
        resolved
    }
}

impl From<image::Rgba<u8>> for RouteStyle {
    fn from(color: image::Rgba<u8>) -> Self {
        Self::new(color)
    }
}

/// the cascade accumulator: concrete values, but with the dash and gap lengths
/// still optional so a later entry can switch the line style back to deriving
/// them
#[derive(Debug, Clone, Copy)]
struct RawRouteStyle {
    /// colour of the line and its arrowheads
    color: image::Rgba<u8>,
    /// line thickness in pixels
    thickness_pixels: f32,
    /// lateral offset in pixels, positive being right of the direction of travel
    offset_pixels: f32,
    /// solid, dashed or dotted
    line_style: RouteLineStyle,
    /// explicit dash length in pixels, or `None` to derive it from the line
    /// style and thickness
    dash_length_pixels: Option<f32>,
    /// explicit gap length in pixels, or `None` to derive it from the line
    /// style and thickness
    gap_length_pixels: Option<f32>,
    /// where arrowheads go
    arrow_placement: RouteArrowPlacement,
    /// multiplier on the thickness-derived arrowhead size
    arrow_scale: f32,
}

impl Default for RawRouteStyle {
    fn default() -> Self {
        Self {
            color: DEFAULT_COLOR,
            thickness_pixels: DEFAULT_THICKNESS_PIXELS,
            offset_pixels: 0.0,
            line_style: RouteLineStyle::Dotted,
            dash_length_pixels: None,
            gap_length_pixels: None,
            arrow_placement: RouteArrowPlacement::default(),
            arrow_scale: DEFAULT_ARROW_SCALE,
        }
    }
}

impl RawRouteStyle {
    /// overwrite the fields the given section sets, leaving the rest in effect.
    ///
    /// Setting a line style without also setting explicit dash or gap lengths
    /// drops any explicit lengths inherited from an earlier entry, so "make
    /// this section dashed" actually produces dashes.
    const fn apply(&mut self, section: &RouteSectionStyle) {
        if let Some(color) = section.color {
            self.color = color;
        }
        if let Some(thickness_pixels) = section.thickness_pixels {
            self.thickness_pixels = thickness_pixels;
        }
        if let Some(offset_pixels) = section.offset_pixels {
            self.offset_pixels = offset_pixels;
        }
        if let Some(line_style) = section.line_style {
            self.line_style = line_style;
            self.dash_length_pixels = None;
            self.gap_length_pixels = None;
        }
        if let Some(dash_length_pixels) = section.dash_length_pixels {
            self.dash_length_pixels = Some(dash_length_pixels);
        }
        if let Some(gap_length_pixels) = section.gap_length_pixels {
            self.gap_length_pixels = Some(gap_length_pixels);
        }
        if let Some(arrow_placement) = section.arrow_placement {
            self.arrow_placement = arrow_placement;
        }
        if let Some(arrow_scale) = section.arrow_scale {
            self.arrow_scale = arrow_scale;
        }
    }

    /// derive the dash and gap lengths, size the arrowheads and clamp
    /// everything into a range the rasteriser can work with
    fn resolve(&self) -> ResolvedRouteStyle {
        let thickness_pixels = sane_value(
            self.thickness_pixels,
            DEFAULT_THICKNESS_PIXELS,
            MIN_THICKNESS_PIXELS,
            MAX_THICKNESS_PIXELS,
        );
        let (derived_dash, derived_gap) = self.line_style.dash_and_gap_pixels(thickness_pixels);
        let dash_length_pixels = sane_value(
            self.dash_length_pixels.unwrap_or(derived_dash),
            derived_dash,
            0.0,
            MAX_DASH_PIXELS,
        );
        let gap_length_pixels = sane_value(
            self.gap_length_pixels.unwrap_or(derived_gap),
            derived_gap,
            0.0,
            MAX_DASH_PIXELS,
        );
        let arrow_scale = sane_value(self.arrow_scale, DEFAULT_ARROW_SCALE, 0.0, MAX_ARROW_SCALE);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the thickness was just clamped into [1, 256], so rounding it fits a u32"
        )]
        let stamp_size = thickness_pixels.round().max(MIN_THICKNESS_PIXELS) as u32;
        ResolvedRouteStyle {
            color: self.color,
            stamp_size,
            offset_pixels: sane_value(
                self.offset_pixels,
                0.0,
                -MAX_OFFSET_PIXELS,
                MAX_OFFSET_PIXELS,
            ),
            dash_length_pixels,
            gap_length_pixels,
            arrow_placement: self.arrow_placement,
            arrow_length_pixels: ARROW_LENGTH_PER_THICKNESS * thickness_pixels * arrow_scale,
            arrow_half_width_pixels: ARROW_HALF_WIDTH_PER_THICKNESS
                * thickness_pixels
                * arrow_scale,
        }
    }
}

/// clamp `value` into `[min, max]`, falling back to `fallback` when it is not
/// a finite number
const fn sane_value(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

/// a fully resolved route style with no options left to interpret, as handed
/// to the rasteriser for one leg of the route
#[expect(
    clippy::module_name_repetitions,
    reason = "this is the resolved counterpart of RouteStyle, the module's primary type, and reads worse under any other name"
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedRouteStyle {
    /// colour of the line and its arrowheads
    pub color: image::Rgba<u8>,
    /// side length in pixels of the square stamped along the line
    pub stamp_size: u32,
    /// lateral offset in pixels, positive being right of the direction of travel
    pub offset_pixels: f32,
    /// length in pixels of each drawn run
    pub dash_length_pixels: f32,
    /// length in pixels of each gap; zero means one run covering the whole leg
    pub gap_length_pixels: f32,
    /// where arrowheads go
    pub arrow_placement: RouteArrowPlacement,
    /// arrowhead length in pixels from tip to base
    pub arrow_length_pixels: f32,
    /// arrowhead half width in pixels, measured from its centre line
    pub arrow_half_width_pixels: f32,
}

#[cfg(test)]
mod test {
    #![expect(
        clippy::float_cmp,
        reason = "the resolved lengths are exact constants produced by exact arithmetic on exact inputs, so an exact comparison is the assertion we want"
    )]

    use super::{
        DEFAULT_COLOR, RouteArrowPlacement, RouteLineStyle, RouteSectionStyle, RouteStyle,
    };
    use pretty_assertions::assert_eq;

    /// a distinctive non-default colour for cascade tests
    const BLUE: image::Rgba<u8> = image::Rgba([0, 0, 255, 255]);

    /// a second distinctive colour for cascade tests
    const GREEN: image::Rgba<u8> = image::Rgba([0, 255, 0, 255]);

    /// resolve a style over `waypoint_count` waypoints, for brevity in tests
    fn resolve(style: &RouteStyle, waypoint_count: usize) -> Vec<super::ResolvedRouteStyle> {
        style.effective_styles_per_waypoint(waypoint_count)
    }

    /// The defaults must reproduce the look the renderer had before per-section
    /// styling existed: red, 3 px dotted with a 3 px gap, no offset, arrows
    /// 15 px long with a 5 px half width on every waypoint.
    #[test]
    fn test_defaults_reproduce_the_legacy_look() {
        let resolved = resolve(&RouteStyle::default(), 3);
        assert_eq!(resolved.len(), 3);
        for style in &resolved {
            assert_eq!(style.color, DEFAULT_COLOR, "default colour");
            assert_eq!(style.stamp_size, 3, "default thickness");
            assert_eq!(style.dash_length_pixels, 3.0, "default dash");
            assert_eq!(style.gap_length_pixels, 3.0, "default gap");
            assert_eq!(style.offset_pixels, 0.0, "default offset");
            assert_eq!(style.arrow_length_pixels, 15.0, "default arrow length");
            assert_eq!(
                style.arrow_half_width_pixels, 5.0,
                "default arrow half width"
            );
            assert_eq!(
                style.arrow_placement,
                RouteArrowPlacement::EveryNthWaypoint(std::num::NonZeroU32::MIN),
                "default arrow placement"
            );
        }
    }

    /// `RouteStyle::new(color)` is just a waypoint 0 entry setting the colour.
    #[test]
    fn test_new_sets_the_route_wide_colour() {
        let resolved = resolve(&RouteStyle::new(BLUE), 2);
        for style in &resolved {
            assert_eq!(
                style.color, BLUE,
                "every waypoint uses the route-wide colour"
            );
        }
    }

    /// An entry at waypoint 0 is the route-wide style; there is no separate
    /// globals concept.
    #[test]
    fn test_entry_at_waypoint_zero_sets_the_route_wide_style() {
        let style = RouteStyle::default().with_section(RouteSectionStyle {
            thickness_pixels: Some(9.0),
            ..RouteSectionStyle::new(0)
        });
        for resolved in &resolve(&style, 4) {
            assert_eq!(
                resolved.stamp_size, 9,
                "the waypoint 0 entry applies throughout"
            );
        }
    }

    /// The cascade must inherit from the style currently in effect, which is
    /// the *previous section*, not the route-wide entry. A section that only
    /// changes the thickness keeps the colour an earlier section set.
    #[test]
    fn test_section_cascade_inherits_from_the_previous_section() {
        let style = RouteStyle::new(GREEN)
            .with_section(RouteSectionStyle {
                color: Some(BLUE),
                ..RouteSectionStyle::new(2)
            })
            .with_section(RouteSectionStyle {
                thickness_pixels: Some(7.0),
                ..RouteSectionStyle::new(4)
            });
        let resolved = resolve(&style, 6);
        assert_eq!(resolved.first().map(|s| s.color), Some(GREEN));
        assert_eq!(resolved.get(2).map(|s| s.color), Some(BLUE));
        let at_four = resolved.get(4).copied();
        assert_eq!(
            at_four.map(|s| s.color),
            Some(BLUE),
            "the thickness-only section must keep the previous section's colour"
        );
        assert_eq!(at_four.map(|s| s.stamp_size), Some(7));
    }

    /// A change takes effect at its own waypoint and stays in effect to the end
    /// of the route.
    #[test]
    fn test_a_section_applies_from_its_waypoint_to_the_end() {
        let style = RouteStyle::new(GREEN).with_section(RouteSectionStyle {
            color: Some(BLUE),
            ..RouteSectionStyle::new(3)
        });
        let resolved = resolve(&style, 6);
        for (index, entry) in resolved.iter().enumerate() {
            let expected = if index >= 3 { BLUE } else { GREEN };
            assert_eq!(entry.color, expected, "waypoint {index}");
        }
    }

    /// Entries need not be added in waypoint order.
    #[test]
    fn test_sections_added_out_of_order_resolve_by_waypoint() {
        let style = RouteStyle::new(GREEN)
            .with_section(RouteSectionStyle {
                color: Some(BLUE),
                ..RouteSectionStyle::new(4)
            })
            .with_section(RouteSectionStyle {
                thickness_pixels: Some(5.0),
                ..RouteSectionStyle::new(2)
            });
        let resolved = resolve(&style, 6);
        assert_eq!(resolved.get(2).map(|s| s.stamp_size), Some(5));
        assert_eq!(resolved.get(2).map(|s| s.color), Some(GREEN));
        assert_eq!(resolved.get(4).map(|s| s.color), Some(BLUE));
        assert_eq!(
            resolved.get(4).map(|s| s.stamp_size),
            Some(5),
            "the earlier section's thickness is still in effect"
        );
    }

    /// Two entries at the same waypoint apply in declaration order, so the last
    /// one added wins.
    #[test]
    fn test_last_section_with_the_same_waypoint_wins() {
        let style = RouteStyle::default()
            .with_section(RouteSectionStyle {
                color: Some(GREEN),
                ..RouteSectionStyle::new(2)
            })
            .with_section(RouteSectionStyle {
                color: Some(BLUE),
                ..RouteSectionStyle::new(2)
            });
        assert_eq!(resolve(&style, 4).get(2).map(|s| s.color), Some(BLUE));
    }

    /// An entry past the end of the route is simply never reached.
    #[test]
    fn test_sections_past_the_last_waypoint_are_ignored() {
        let style = RouteStyle::new(GREEN).with_section(RouteSectionStyle {
            color: Some(BLUE),
            ..RouteSectionStyle::new(99)
        });
        for entry in &resolve(&style, 4) {
            assert_eq!(
                entry.color, GREEN,
                "the out-of-range section must not apply"
            );
        }
    }

    /// Dash and gap follow the line style and scale with the thickness when
    /// they are not set explicitly.
    #[test]
    fn test_derived_dash_and_gap_scale_with_thickness() {
        let dotted = RouteStyle::default()
            .with_thickness(9.0)
            .with_line_style(RouteLineStyle::Dotted);
        assert_eq!(
            resolve(&dotted, 1)
                .first()
                .map(|s| (s.dash_length_pixels, s.gap_length_pixels)),
            Some((9.0, 9.0))
        );
        let dashed = RouteStyle::default()
            .with_thickness(3.0)
            .with_line_style(RouteLineStyle::Dashed);
        assert_eq!(
            resolve(&dashed, 1)
                .first()
                .map(|s| (s.dash_length_pixels, s.gap_length_pixels)),
            Some((12.0, 6.0))
        );
        let solid = RouteStyle::default().with_line_style(RouteLineStyle::Solid);
        assert_eq!(
            resolve(&solid, 1).first().map(|s| s.gap_length_pixels),
            Some(0.0),
            "solid means no gap"
        );
    }

    /// Setting an explicit dash length overrides what the line style derives.
    #[test]
    fn test_explicit_dash_overrides_the_derived_one() {
        let style = RouteStyle::default().with_section(RouteSectionStyle {
            line_style: Some(RouteLineStyle::Dashed),
            dash_length_pixels: Some(2.0),
            ..RouteSectionStyle::new(0)
        });
        let resolved = resolve(&style, 1);
        assert_eq!(resolved.first().map(|s| s.dash_length_pixels), Some(2.0));
        assert_eq!(
            resolved.first().map(|s| s.gap_length_pixels),
            Some(6.0),
            "the gap still comes from the line style"
        );
    }

    /// Switching the line style in a later section drops explicit lengths an
    /// earlier section set, so "make this bit dashed" really produces dashes.
    #[test]
    fn test_changing_line_style_drops_inherited_explicit_lengths() {
        let style = RouteStyle::default()
            .with_section(RouteSectionStyle {
                dash_length_pixels: Some(1.0),
                gap_length_pixels: Some(1.0),
                ..RouteSectionStyle::new(0)
            })
            .with_section(RouteSectionStyle {
                line_style: Some(RouteLineStyle::Dashed),
                ..RouteSectionStyle::new(2)
            });
        let resolved = resolve(&style, 4);
        assert_eq!(
            resolved.first().map(|s| s.dash_length_pixels),
            Some(1.0),
            "the explicit length applies before the line style change"
        );
        assert_eq!(
            resolved
                .get(2)
                .map(|s| (s.dash_length_pixels, s.gap_length_pixels)),
            Some((12.0, 6.0)),
            "the line style change re-derives both lengths"
        );
    }

    /// Arrow size follows the thickness so a thicker line gets bigger arrows,
    /// and the scale multiplies on top of that.
    #[test]
    fn test_arrow_size_follows_thickness_and_scale() {
        let thick = RouteStyle::default().with_thickness(6.0);
        assert_eq!(
            resolve(&thick, 1)
                .first()
                .map(|s| (s.arrow_length_pixels, s.arrow_half_width_pixels)),
            Some((30.0, 10.0)),
            "doubling the thickness doubles the arrows"
        );
        let scaled = RouteStyle::default().with_section(RouteSectionStyle {
            arrow_scale: Some(2.0),
            ..RouteSectionStyle::new(0)
        });
        assert_eq!(
            resolve(&scaled, 1).first().map(|s| s.arrow_length_pixels),
            Some(30.0)
        );
    }

    /// Nonsense values must be clamped to something drawable rather than
    /// reaching the rasteriser as a NaN or a negative size.
    #[test]
    fn test_invalid_values_are_clamped() -> Result<(), Box<dyn std::error::Error>> {
        let style = RouteStyle::default().with_section(RouteSectionStyle {
            thickness_pixels: Some(f32::NAN),
            offset_pixels: Some(f32::INFINITY),
            dash_length_pixels: Some(-5.0),
            gap_length_pixels: Some(f32::NEG_INFINITY),
            arrow_scale: Some(1e30),
            ..RouteSectionStyle::new(0)
        });
        let resolved = resolve(&style, 1)
            .first()
            .copied()
            .ok_or("a one-waypoint route must resolve one style")?;
        assert_eq!(
            resolved.stamp_size, 3,
            "NaN thickness falls back to default"
        );
        assert_eq!(resolved.offset_pixels, 0.0, "infinite offset falls back");
        assert_eq!(
            resolved.dash_length_pixels, 0.0,
            "negative dash clamps to 0"
        );
        assert_eq!(
            resolved.gap_length_pixels, 3.0,
            "an infinite gap falls back to the derived one"
        );
        assert!(
            resolved.arrow_length_pixels.is_finite(),
            "a huge arrow scale must stay finite"
        );
        Ok(())
    }

    /// A very large thickness must still produce a usable stamp size.
    #[test]
    fn test_thickness_is_clamped_to_a_drawable_range() {
        let huge = RouteStyle::default().with_thickness(1e9);
        assert_eq!(resolve(&huge, 1).first().map(|s| s.stamp_size), Some(256));
        let tiny = RouteStyle::default().with_thickness(0.01);
        assert_eq!(resolve(&tiny, 1).first().map(|s| s.stamp_size), Some(1));
    }

    /// An empty style resolves to the defaults for every waypoint, and a zero
    /// waypoint route resolves to nothing at all.
    #[test]
    fn test_empty_style_and_empty_route() {
        assert!(resolve(&RouteStyle::default(), 0).is_empty());
        assert_eq!(resolve(&RouteStyle::default(), 1).len(), 1);
    }

    /// `base_mut` must reuse the waypoint 0 entry rather than stacking up a new
    /// one per call.
    #[test]
    fn test_base_mut_reuses_the_waypoint_zero_entry() -> Result<(), Box<dyn std::error::Error>> {
        let style = RouteStyle::new(BLUE).with_thickness(5.0).with_offset(4.0);
        assert_eq!(
            style.sections().len(),
            1,
            "the route-wide controls all write into one entry"
        );
        let resolved = resolve(&style, 1)
            .first()
            .copied()
            .ok_or("a one-waypoint route must resolve one style")?;
        assert_eq!(resolved.color, BLUE);
        assert_eq!(resolved.stamp_size, 5);
        assert_eq!(resolved.offset_pixels, 4.0);
        Ok(())
    }
}
