//! Map-related data types

#[cfg(feature = "chumsky")]
use chumsky::{
    IterParser as _, Parser,
    prelude::{any, just},
    text::whitespace,
};

#[cfg(feature = "chumsky")]
use crate::utils::{
    f32_parser, i16_parser, i32_parser, u8_parser, u16_parser, u32_parser,
    url_text_component_parser,
};

/// represents a Second Life distance in meters
#[derive(Debug, Clone, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Distance(f64);

impl Distance {
    /// creates a distance from a value in meters
    #[must_use]
    pub const fn new(meters: f64) -> Self {
        Self(meters)
    }

    /// the distance in meters
    #[must_use]
    pub const fn meters(&self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for Distance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} m", self.0)
    }
}

impl std::ops::Add for Distance {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Distance {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul<u8> for Distance {
    type Output = Self;

    fn mul(self, rhs: u8) -> Self::Output {
        Self(self.0 * f64::from(rhs))
    }
}

impl std::ops::Mul<u16> for Distance {
    type Output = Self;

    fn mul(self, rhs: u16) -> Self::Output {
        Self(self.0 * f64::from(rhs))
    }
}

impl std::ops::Mul<u32> for Distance {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self(self.0 * f64::from(rhs))
    }
}

impl std::ops::Mul<f32> for Distance {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * f64::from(rhs))
    }
}

impl std::ops::Mul<f64> for Distance {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl std::ops::Div<u8> for Distance {
    type Output = Self;

    fn div(self, rhs: u8) -> Self::Output {
        Self(self.0 / f64::from(rhs))
    }
}

impl std::ops::Div<u16> for Distance {
    type Output = Self;

    fn div(self, rhs: u16) -> Self::Output {
        Self(self.0 / f64::from(rhs))
    }
}

impl std::ops::Div<u32> for Distance {
    type Output = Self;

    fn div(self, rhs: u32) -> Self::Output {
        Self(self.0 / f64::from(rhs))
    }
}

impl std::ops::Div<f32> for Distance {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / f64::from(rhs))
    }
}

impl std::ops::Div<f64> for Distance {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl std::ops::Div for Distance {
    type Output = f64;

    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl std::ops::Rem<u8> for Distance {
    type Output = Self;

    fn rem(self, rhs: u8) -> Self::Output {
        Self(self.0 % f64::from(rhs))
    }
}

impl std::ops::Rem<u16> for Distance {
    type Output = Self;

    fn rem(self, rhs: u16) -> Self::Output {
        Self(self.0 % f64::from(rhs))
    }
}

impl std::ops::Rem<u32> for Distance {
    type Output = Self;

    fn rem(self, rhs: u32) -> Self::Output {
        Self(self.0 % f64::from(rhs))
    }
}

impl std::ops::Rem<f32> for Distance {
    type Output = Self;

    fn rem(self, rhs: f32) -> Self::Output {
        Self(self.0 % f64::from(rhs))
    }
}

impl std::ops::Rem<f64> for Distance {
    type Output = Self;

    fn rem(self, rhs: f64) -> Self::Output {
        Self(self.0 % rhs)
    }
}

/// parse a distance
///
/// "235.23 m"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn distance_parser<'src>()
-> impl Parser<'src, &'src str, Distance, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    crate::utils::unsigned_f64_parser()
        .then_ignore(whitespace().or_not())
        .then_ignore(just('m'))
        .map(Distance)
}

/// A Second Life land area, in **square metres** — the unit SL measures parcels
/// and land-tier accounting in (a member's group land contribution, a parcel's
/// actual/billable area, an avatar's land credit/commitment, …).
///
/// This is deliberately **not** an [`LindenAmount`](crate::money::LindenAmount):
/// land areas occupy the same signed-32-bit integer slots prices use, and the
/// two are trivially confusable as raw integers. Wrapping area in its own
/// newtype makes "passed a land area where an L$ price was expected" (and
/// vice-versa) a compile error. A land area is non-negative by construction.
#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LandArea(pub u32);

impl LandArea {
    /// A zero land area.
    pub const ZERO: Self = Self(0);

    /// The wrapped count of square metres.
    #[must_use]
    pub const fn get(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for LandArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(value) = self;
        write!(f, "{value} m²")
    }
}

impl std::ops::Add for LandArea {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let Self(lhs) = self;
        let Self(rhs) = rhs;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the same overflow behaviour as the underlying integer addition, which is what a caller summing areas expects"
        )]
        Self(lhs + rhs)
    }
}

impl std::ops::Sub for LandArea {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let Self(lhs) = self;
        let Self(rhs) = rhs;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the same underflow behaviour as the underlying integer subtraction, which is what a caller differencing areas expects"
        )]
        Self(lhs - rhs)
    }
}

/// parse a land area
///
/// "1024 m²"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn land_area_parser<'src>()
-> impl Parser<'src, &'src str, LandArea, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    u32_parser()
        .then_ignore(whitespace().or_not())
        .then_ignore(just("m²"))
        .map(LandArea)
}

/// Grid coordinates for the position of a region on the map
///
/// the first region, Da Boom is located at 1000, 1000
#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct GridCoordinates {
    /// the x coordinate of the region, this is basically the horizontal
    /// position of the region on the map increasing from west to east
    ///
    /// common values are between roughly 395 and 1358; the type is `u32` (not
    /// `u16`) because the Second Life whole-grid map layer reports rectangle
    /// bounds that can exceed `u16::MAX`
    x: u32,
    /// the y coordinate of the region, this is basically the vertical
    /// position of the region on the map increasing from south to north
    ///
    /// common values are between roughly 479 and 1430; the type is `u32` (not
    /// `u16`) because the Second Life whole-grid map layer reports rectangle
    /// bounds that can exceed `u16::MAX`
    y: u32,
}

impl GridCoordinates {
    /// Create a new `GridCoordinates`
    #[must_use]
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// The x coordinate of the region
    #[must_use]
    pub const fn x(&self) -> u32 {
        self.x
    }

    /// The y coordinate of the region
    #[must_use]
    pub const fn y(&self) -> u32 {
        self.y
    }
}

/// an offset between two `GridCoordinates`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCoordinateOffset {
    /// the offset in the x direction
    x: i32,
    /// the offset in the y direction
    y: i32,
}

impl GridCoordinateOffset {
    /// creates a new `GridCoordinateOffset`
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// the offset in the x direction
    #[must_use]
    pub const fn x(&self) -> i32 {
        self.x
    }

    /// the offset in the y direction
    #[must_use]
    pub const fn y(&self) -> i32 {
        self.y
    }
}

impl std::ops::Add<GridCoordinateOffset> for GridCoordinates {
    type Output = Self;

    fn add(self, rhs: GridCoordinateOffset) -> Self::Output {
        Self::new(
            (i64::from(self.x).saturating_add(i64::from(rhs.x)))
                .try_into()
                .unwrap_or(if rhs.x > 0 { u32::MAX } else { u32::MIN }),
            (i64::from(self.y).saturating_add(i64::from(rhs.y)))
                .try_into()
                .unwrap_or(if rhs.y > 0 { u32::MAX } else { u32::MIN }),
        )
    }
}

impl std::ops::Sub<Self> for GridCoordinates {
    type Output = GridCoordinateOffset;

    fn sub(self, rhs: Self) -> Self::Output {
        /// Saturates the `i64` difference of two `u32` coordinates into the
        /// `i32` an offset holds (real grid differences never approach `i32`'s
        /// range, but the conversion must still be total).
        fn saturate_to_i32(difference: i64) -> i32 {
            difference
                .try_into()
                .unwrap_or(if difference > 0 { i32::MAX } else { i32::MIN })
        }
        GridCoordinateOffset::new(
            saturate_to_i32(i64::from(self.x).saturating_sub(i64::from(rhs.x))),
            saturate_to_i32(i64::from(self.y).saturating_sub(i64::from(rhs.y))),
        )
    }
}

/// represents a rectangle of regions defined by the lower left (minimum coordinates)
/// and upper right (maximum coordinates) corners in `GridCoordinates`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridRectangle {
    /// the lower left (minimum coordinates) corner of the rectangle
    lower_left_corner: GridCoordinates,
    /// the upper right (maximum coordinates) corner of the rectangle
    upper_right_corner: GridCoordinates,
}

impl GridRectangle {
    /// creates a new `GridRectangle` given any two corners
    #[must_use]
    pub fn new(corner1: GridCoordinates, corner2: GridCoordinates) -> Self {
        Self {
            lower_left_corner: GridCoordinates::new(
                corner1.x().min(corner2.x()),
                corner1.y().min(corner2.y()),
            ),
            upper_right_corner: GridCoordinates::new(
                corner1.x().max(corner2.x()),
                corner1.y().max(corner2.y()),
            ),
        }
    }

    /// returns a new `GridRectangle` extended by `by` regions on the west (-x) side
    ///
    /// Saturates at the western edge of the grid (x = 0).
    #[must_use]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "GridCoordinates + GridCoordinateOffset saturates at u32::MIN/u32::MAX"
    )]
    pub fn expanded_west(&self, by: u16) -> Self {
        Self::new(
            self.lower_left_corner.to_owned() + GridCoordinateOffset::new(-i32::from(by), 0),
            self.upper_right_corner.to_owned(),
        )
    }

    /// returns a new `GridRectangle` extended by `by` regions on the east (+x) side
    ///
    /// Saturates at the eastern edge of the grid (x = `u32::MAX`).
    #[must_use]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "GridCoordinates + GridCoordinateOffset saturates at u32::MIN/u32::MAX"
    )]
    pub fn expanded_east(&self, by: u16) -> Self {
        Self::new(
            self.lower_left_corner.to_owned(),
            self.upper_right_corner.to_owned() + GridCoordinateOffset::new(i32::from(by), 0),
        )
    }

    /// returns a new `GridRectangle` extended by `by` regions on the south (-y) side
    ///
    /// Saturates at the southern edge of the grid (y = 0).
    #[must_use]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "GridCoordinates + GridCoordinateOffset saturates at u32::MIN/u32::MAX"
    )]
    pub fn expanded_south(&self, by: u16) -> Self {
        Self::new(
            self.lower_left_corner.to_owned() + GridCoordinateOffset::new(0, -i32::from(by)),
            self.upper_right_corner.to_owned(),
        )
    }

    /// returns a new `GridRectangle` extended by `by` regions on the north (+y) side
    ///
    /// Saturates at the northern edge of the grid (y = `u32::MAX`).
    #[must_use]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "GridCoordinates + GridCoordinateOffset saturates at u32::MIN/u32::MAX"
    )]
    pub fn expanded_north(&self, by: u16) -> Self {
        Self::new(
            self.lower_left_corner.to_owned(),
            self.upper_right_corner.to_owned() + GridCoordinateOffset::new(0, i32::from(by)),
        )
    }
}

/// represents a grid rectangle like type (usually one that contains a
/// grid rectangle or one that contains a corner and is of a known size
pub trait GridRectangleLike {
    /// the `GridRectangle` represented by this map like image
    #[must_use]
    fn grid_rectangle(&self) -> GridRectangle;

    /// returns the lower left corner of the rectangle
    #[must_use]
    fn lower_left_corner(&self) -> GridCoordinates {
        self.grid_rectangle().lower_left_corner().to_owned()
    }

    /// returns the lower right corner of the rectangle
    #[must_use]
    fn lower_right_corner(&self) -> GridCoordinates {
        GridCoordinates::new(
            self.grid_rectangle().upper_right_corner().x(),
            self.grid_rectangle().lower_left_corner().y(),
        )
    }

    /// returns the upper left corner of the rectangle
    #[must_use]
    fn upper_left_corner(&self) -> GridCoordinates {
        GridCoordinates::new(
            self.grid_rectangle().lower_left_corner().x(),
            self.grid_rectangle().upper_right_corner().y(),
        )
    }

    /// returns the upper right corner of the rectangle
    #[must_use]
    fn upper_right_corner(&self) -> GridCoordinates {
        self.grid_rectangle().upper_right_corner().to_owned()
    }

    /// the size of the map like image in regions in the x direction (width)
    #[must_use]
    fn size_x(&self) -> u32 {
        self.grid_rectangle().size_x()
    }

    /// the size of the map like image in regions in the y direction (width)
    #[must_use]
    fn size_y(&self) -> u32 {
        self.grid_rectangle().size_y()
    }

    /// returns a range for the region x coordinates of this rectangle
    #[must_use]
    fn x_range(&self) -> std::ops::RangeInclusive<u32> {
        self.lower_left_corner().x()..=self.upper_right_corner().x()
    }

    /// returns a range for the region y coordinates of this rectangle
    #[must_use]
    fn y_range(&self) -> std::ops::RangeInclusive<u32> {
        self.lower_left_corner().y()..=self.upper_right_corner().y()
    }

    /// checks if a given set of `GridCoordinates` is within this `GridRectangle`
    #[must_use]
    fn contains(&self, grid_coordinates: &GridCoordinates) -> bool {
        self.lower_left_corner().x() <= grid_coordinates.x()
            && grid_coordinates.x() <= self.upper_right_corner().x()
            && self.lower_left_corner().y() <= grid_coordinates.y()
            && grid_coordinates.y() <= self.upper_right_corner().y()
    }

    /// returns a new `GridRectangle` which is the area where this `GridRectangle`
    /// and another intersect each other or None if there is no intersection
    #[must_use]
    fn intersect<O>(&self, other: &O) -> Option<GridRectangle>
    where
        O: GridRectangleLike,
    {
        let self_x_range: ranges::GenericRange<u32> = self.x_range().into();
        let self_y_range: ranges::GenericRange<u32> = self.y_range().into();
        let other_x_range: ranges::GenericRange<u32> = other.x_range().into();
        let other_y_range: ranges::GenericRange<u32> = other.y_range().into();
        let x_intersection = self_x_range.intersect(other_x_range);
        let y_intersection = self_y_range.intersect(other_y_range);
        match (x_intersection, y_intersection) {
            (
                ranges::OperationResult::Single(x_range),
                ranges::OperationResult::Single(y_range),
            ) => {
                use std::ops::Bound;
                use std::ops::RangeBounds as _;
                match (
                    x_range.start_bound(),
                    x_range.end_bound(),
                    y_range.start_bound(),
                    y_range.end_bound(),
                ) {
                    (
                        Bound::Included(start_x),
                        Bound::Included(end_x),
                        Bound::Included(start_y),
                        Bound::Included(end_y),
                    ) => Some(GridRectangle::new(
                        GridCoordinates::new(*start_x, *start_y),
                        GridCoordinates::new(*end_x, *end_y),
                    )),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// returns a PPS HUD description string for this `GridRectangle`
    ///
    /// The PPS HUD is a map HUD commonly used in the SL sailing community
    /// and usually you need to configure it by clicking on the HUD while
    /// you are at the matching location in-world to calibrate the coordinates
    /// on the map texture.
    ///
    /// This string needs to be put in the description of the PPS HUD
    /// dot prim with "Edit linked objects" to avoid the need for manual
    /// calibration.
    #[must_use]
    fn pps_hud_config(&self) -> String {
        // the lower left corner as an LSL vector of metres from the grid
        // coordinate origin (`<256 * grid_x, 256 * grid_y, 0>`)
        let lower_left_corner = GlobalCoordinates::from_grid_corner(self.lower_left_corner());
        // this is the lower left corner as an LSL vector of meters from the grid coordinate origin
        // followed by the width and height of the map in regions
        // and a 0/1 for the locked state of the HUD
        // each of those is separated from the next by a slash character
        format!(
            "<{},{},0>/{}/{}/1",
            lower_left_corner.x(),
            lower_left_corner.y(),
            f64::from(self.size_x()),
            f64::from(self.size_y())
        )
    }
}

impl GridRectangleLike for GridRectangle {
    fn grid_rectangle(&self) -> GridRectangle {
        self.to_owned()
    }

    fn lower_left_corner(&self) -> GridCoordinates {
        self.lower_left_corner.to_owned()
    }

    fn upper_right_corner(&self) -> GridCoordinates {
        self.upper_right_corner.to_owned()
    }

    fn size_x(&self) -> u32 {
        self.upper_right_corner
            .x()
            .saturating_sub(self.lower_left_corner().x())
            .saturating_add(1)
    }

    fn size_y(&self) -> u32 {
        self.upper_right_corner
            .y()
            .saturating_sub(self.lower_left_corner().y())
            .saturating_add(1)
    }

    fn x_range(&self) -> std::ops::RangeInclusive<u32> {
        self.lower_left_corner.x()..=self.upper_right_corner.x()
    }

    fn y_range(&self) -> std::ops::RangeInclusive<u32> {
        self.lower_left_corner.y()..=self.upper_right_corner.y()
    }
}

impl GridRectangleLike for MapTileDescriptor {
    fn grid_rectangle(&self) -> GridRectangle {
        GridRectangle::new(
            self.lower_left_corner,
            GridCoordinates::new(
                self.lower_left_corner
                    .x()
                    .saturating_add(u32::from(self.zoom_level.tile_size()))
                    .saturating_sub(1),
                self.lower_left_corner
                    .y()
                    .saturating_add(u32::from(self.zoom_level.tile_size()))
                    .saturating_sub(1),
            ),
        )
    }
}

/// A trait to allow adding methods to `Vec<GridCoordinates>`
pub trait GridCoordinatesExt {
    /// returns the coordinates of the lower left corner and the coordinates of
    /// the upper right corner of a rectangle of regions containing all the grid
    /// coordinates in this container
    ///
    /// returns None if the container is empty
    fn bounding_rectangle(&self) -> Option<GridRectangle>;
}

impl GridCoordinatesExt for Vec<GridCoordinates> {
    fn bounding_rectangle(&self) -> Option<GridRectangle> {
        if self.is_empty() {
            return None;
        }
        let (xs, ys): (Vec<u32>, Vec<u32>) = self.iter().map(|gc| (gc.x(), gc.y())).unzip();
        // unwrap is okay in these cases because we checked above that the container is non-empty
        #[expect(
            clippy::unwrap_used,
            reason = "we checked above that the container is non-empty"
        )]
        let (min_x, max_x) = (xs.iter().min().unwrap(), xs.iter().max().unwrap());
        #[expect(
            clippy::unwrap_used,
            reason = "we checked above that the container is non-empty"
        )]
        let (min_y, max_y) = (ys.iter().min().unwrap(), ys.iter().max().unwrap());
        Some(GridRectangle {
            lower_left_corner: GridCoordinates::new(*min_x, *min_y),
            upper_right_corner: GridCoordinates::new(*max_x, *max_y),
        })
    }
}

/// Region coordinates for the position of something inside a region
///
/// Usually limited to 0..256 for x and y and 0..4096 for z (height)
/// but values outside those ranges are possible for positions of objects
/// in the process of crossing from one region to another or in similar
/// situations where they belong to one simulator logically but are located
/// outside of that simulator's region
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct RegionCoordinates {
    /// the x coordinate inside the region from the western edge (0) to the
    /// eastern edge (256)
    x: f32,
    /// the y coordinate inside the region from the southern edge (0) to the
    /// northern edge (256)
    y: f32,
    /// the z coordinate inside the region from the bottom (0) to the top (4096)
    /// higher values are possible but for objects can not be rezzed above 4096m
    /// and teleports are clamped to that as well
    z: f32,
}

/// parse region coordinates
///
/// "{ 1.234, 2.345, 3.456 }"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn region_coordinates_parser<'src>()
-> impl Parser<'src, &'src str, RegionCoordinates, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just('{')
        .ignore_then(whitespace().or_not())
        .ignore_then(f32_parser())
        .then_ignore(just(','))
        .then_ignore(whitespace().or_not())
        .then(f32_parser())
        .then_ignore(just(','))
        .then_ignore(whitespace().or_not())
        .then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just('}'))
        .map(|((x, y), z)| RegionCoordinates::new(x, y, z))
}

impl RegionCoordinates {
    /// Create a new `RegionCoordinates`
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The x coordinate inside the region
    #[must_use]
    pub const fn x(&self) -> f32 {
        self.x
    }

    /// The y coordinate inside the region
    #[must_use]
    pub const fn y(&self) -> f32 {
        self.y
    }

    /// The z coordinate inside the region
    #[must_use]
    pub const fn z(&self) -> f32 {
        self.z
    }

    /// checks if the coordinates are within bounds
    #[must_use]
    pub fn in_bounds(&self) -> bool {
        self.x >= 0f32
            && self.x < 256f32
            && self.y >= 0f32
            && self.y < 256f32
            && self.z >= 0f32
            && self.z < 4096f32
    }
}

impl From<crate::lsl::Vector> for RegionCoordinates {
    fn from(value: crate::lsl::Vector) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

/// The number of metres along one axis of a region; a grid-index step.
const REGION_SIZE_METERS: f64 = 256.0;

/// A 3-D facing direction — the direction an avatar faces, as carried by the
/// various `look_at` fields (the viewer's agent/camera *at*-axis). It is a
/// direction, **not** a position: the wire stores three `f32`s and the viewer
/// uses the full 3-D vector (including any vertical component) as the forward
/// axis. It is conventionally a unit vector, but the wire does not enforce
/// normalisation, so the raw components are preserved verbatim for byte-identical
/// round-trips.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Direction {
    /// The x component of the facing direction.
    x: f32,
    /// The y component of the facing direction.
    y: f32,
    /// The z component of the facing direction.
    z: f32,
}

impl Direction {
    /// A zero direction (the wire `(0, 0, 0)` sentinel the viewer replaces with
    /// the current camera axis).
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Creates a direction from its raw components, without normalising.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The x component of the facing direction.
    #[must_use]
    pub const fn x(&self) -> f32 {
        self.x
    }

    /// The y component of the facing direction.
    #[must_use]
    pub const fn y(&self) -> f32 {
        self.y
    }

    /// The z component of the facing direction.
    #[must_use]
    pub const fn z(&self) -> f32 {
        self.z
    }

    /// The Euclidean length (magnitude) of the direction vector.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.z
            .mul_add(self.z, self.x.mul_add(self.x, self.y * self.y))
            .sqrt()
    }

    /// The unit-length direction, or `None` when the vector has (near-)zero
    /// length and a direction is therefore undefined.
    #[must_use]
    pub fn normalized(&self) -> Option<Self> {
        let length = self.length();
        if length > f32::EPSILON {
            Some(Self::new(self.x / length, self.y / length, self.z / length))
        } else {
            None
        }
    }
}

/// A grid-global position in metres — the viewer's `LLVector3d` "global" frame,
/// where the value along an axis is `region_grid_index * 256 + region_local`.
/// Held as `f64` to match the wire's double-precision global vectors (the
/// directory/event/pick replies carry `LLVector3d`); the few replies that send
/// a single-precision global position widen to `f64` at the codec boundary.
///
/// `sl-types` also has region-local ([`RegionCoordinates`]) and region-index
/// ([`GridCoordinates`]) coordinates; this is the global-metre coordinate that
/// relates the two.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GlobalCoordinates {
    /// The global x coordinate, in metres (west→east).
    x: f64,
    /// The global y coordinate, in metres (south→north).
    y: f64,
    /// The global z coordinate, in metres (altitude).
    z: f64,
}

impl GlobalCoordinates {
    /// Creates global coordinates from their raw metre components.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The global x coordinate, in metres.
    #[must_use]
    pub const fn x(&self) -> f64 {
        self.x
    }

    /// The global y coordinate, in metres.
    #[must_use]
    pub const fn y(&self) -> f64 {
        self.y
    }

    /// The global z coordinate, in metres.
    #[must_use]
    pub const fn z(&self) -> f64 {
        self.z
    }

    /// Combines a region's grid index and a region-local position into a global
    /// position (`grid_index * 256 + region_local`). The inverse of
    /// [`split`](Self::split).
    #[must_use]
    pub fn from_grid_and_region(grid: GridCoordinates, region: RegionCoordinates) -> Self {
        Self {
            x: f64::from(grid.x()).mul_add(REGION_SIZE_METERS, f64::from(region.x())),
            y: f64::from(grid.y()).mul_add(REGION_SIZE_METERS, f64::from(region.y())),
            z: f64::from(region.z()),
        }
    }

    /// The grid-global position of a region's south-west **corner** — its
    /// `grid_index * 256` origin at zero altitude. This is the corner the PPS
    /// HUD config uses (`<256 * grid_x, 256 * grid_y, 0>`); it avoids
    /// constructing a throwaway all-zero [`RegionCoordinates`] just to reach
    /// [`from_grid_and_region`](Self::from_grid_and_region).
    #[must_use]
    pub fn from_grid_corner(grid: GridCoordinates) -> Self {
        Self {
            x: f64::from(grid.x()) * REGION_SIZE_METERS,
            y: f64::from(grid.y()) * REGION_SIZE_METERS,
            z: 0.0,
        }
    }

    /// Splits a global position into the containing region's grid index and the
    /// region-local position within it. The inverse of
    /// [`from_grid_and_region`](Self::from_grid_and_region).
    ///
    /// Returns `None` when the global position falls outside the representable
    /// grid (a negative or out-of-`u32`-range region index), which never
    /// happens for a position the grid actually sent.
    #[must_use]
    pub fn split(&self) -> Option<(GridCoordinates, RegionCoordinates)> {
        let grid_x = region_index(self.x)?;
        let grid_y = region_index(self.y)?;
        let local_x = f64::from(grid_x).mul_add(-REGION_SIZE_METERS, self.x);
        let local_y = f64::from(grid_y).mul_add(-REGION_SIZE_METERS, self.y);
        Some((
            GridCoordinates::new(grid_x, grid_y),
            RegionCoordinates::new(narrow(local_x), narrow(local_y), narrow(self.z)),
        ))
    }
}

impl From<(GridCoordinates, RegionCoordinates)> for GlobalCoordinates {
    fn from((grid, region): (GridCoordinates, RegionCoordinates)) -> Self {
        Self::from_grid_and_region(grid, region)
    }
}

impl From<GridCoordinates> for GlobalCoordinates {
    /// Builds the south-west corner of the region (see
    /// [`from_grid_corner`](Self::from_grid_corner)).
    fn from(grid: GridCoordinates) -> Self {
        Self::from_grid_corner(grid)
    }
}

/// The region grid index containing a global-metre coordinate, or `None` when
/// it falls outside the `0..=u32::MAX` grid range (including a non-finite or
/// negative input).
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the floored index is checked finite and within u32 range before the cast"
)]
fn region_index(meters: f64) -> Option<u32> {
    let index = (meters / REGION_SIZE_METERS).floor();
    if index.is_finite() && index >= 0.0 && index <= f64::from(u32::MAX) {
        Some(index as u32)
    } else {
        None
    }
}

/// Narrows a region-local-metre `f64` to the `f32` a region-local coordinate
/// uses. A region-local offset is a small (0..256) in-range metre value, so the
/// narrowing is exact for the values the grid sends.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "a region-local offset is a small (0..256) in-range metre value"
)]
const fn narrow(meters: f64) -> f32 {
    meters as f32
}

/// A 3-axis **scale factor** (X/Y/Z) — a dimensionless multiplier per axis,
/// **not** a size in metres. Its one wire user is the water normal-map
/// "Reflection Wavelet Scale" (the viewer's `normal_scale`: three per-axis
/// multipliers applied to the wavelet normal-map sampling). A scale is not a
/// position or a direction (it has no origin and need not be a unit vector), so
/// it gets its own type.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Scale {
    /// The x-axis scale factor.
    x: f32,
    /// The y-axis scale factor.
    y: f32,
    /// The z-axis scale factor.
    z: f32,
}

impl Scale {
    /// Creates a scale from its per-axis factors.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The x-axis scale factor.
    #[must_use]
    pub const fn x(&self) -> f32 {
        self.x
    }

    /// The y-axis scale factor.
    #[must_use]
    pub const fn y(&self) -> f32 {
        self.y
    }

    /// The z-axis scale factor.
    #[must_use]
    pub const fn z(&self) -> f32 {
        self.z
    }
}

/// The flags describing how and why a teleport happened, carried by
/// `TeleportFinish` (and `TeleportProgress`) as the `TeleportFlags` U32
/// bitfield. Mirrors the reference viewer's `TELEPORT_FLAGS_*`
/// (`indra/llmessage/llteleportflags.h`).
///
/// Note: OpenSim collapses the flags it sends on `TeleportFinish` to
/// [`VIA_LOCATION`](Self::VIA_LOCATION) (plus [`IS_FLYING`](Self::IS_FLYING)),
/// so the full set of `VIA_*` reasons is only observable on the Second Life
/// grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeleportFlags(pub u32);

impl TeleportFlags {
    /// Set the agent's home to the teleport target (`SET_HOME_TO_TARGET`, a
    /// newbie leaving the prelude).
    pub const SET_HOME_TO_TARGET: u32 = 1 << 0;
    /// Set the agent's last location to the target (`SET_LAST_TO_TARGET`).
    pub const SET_LAST_TO_TARGET: u32 = 1 << 1;
    /// Teleport via a lure / teleport offer (`VIA_LURE`).
    pub const VIA_LURE: u32 = 1 << 2;
    /// Teleport via a landmark (`VIA_LANDMARK`).
    pub const VIA_LANDMARK: u32 = 1 << 3;
    /// Teleport via an explicit location (`VIA_LOCATION`).
    pub const VIA_LOCATION: u32 = 1 << 4;
    /// Teleport to the agent's home (`VIA_HOME`).
    pub const VIA_HOME: u32 = 1 << 5;
    /// Teleport via a telehub (`VIA_TELEHUB`).
    pub const VIA_TELEHUB: u32 = 1 << 6;
    /// Teleport as part of logging in (`VIA_LOGIN`).
    pub const VIA_LOGIN: u32 = 1 << 7;
    /// Teleport via a godlike lure (`VIA_GODLIKE_LURE`).
    pub const VIA_GODLIKE_LURE: u32 = 1 << 8;
    /// The teleport was performed with god powers (`GODLIKE`).
    pub const GODLIKE: u32 = 1 << 9;
    /// An emergency ("911") teleport (`FLAGS_911`).
    pub const NINE_ONE_ONE: u32 = 1 << 10;
    /// Cancelling the teleport is disabled (`DISABLE_CANCEL`, used by
    /// `llTeleportAgentHome`).
    pub const DISABLE_CANCEL: u32 = 1 << 11;
    /// Teleport via a region id (`VIA_REGION_ID`).
    pub const VIA_REGION_ID: u32 = 1 << 12;
    /// The agent was flying when the teleport started (`IS_FLYING`).
    pub const IS_FLYING: u32 = 1 << 13;
    /// Show the reset-home UI on arrival (`SHOW_RESET_HOME`).
    pub const SHOW_RESET_HOME: u32 = 1 << 14;
    /// Force a redirect to some location (`FORCE_REDIRECT`, used when kicking
    /// someone from land).
    pub const FORCE_REDIRECT: u32 = 1 << 15;
    /// Teleport via global coordinates (`VIA_GLOBAL_COORDS`).
    pub const VIA_GLOBAL_COORDS: u32 = 1 << 16;
    /// The teleport stays within the same region (`WITHIN_REGION`).
    pub const WITHIN_REGION: u32 = 1 << 17;

    /// Whether all of the bits in `mask` are set.
    #[must_use]
    pub const fn contains(self, mask: u32) -> bool {
        self.0 & mask == mask
    }
}

/// The name of a region
#[nutype::nutype(
    sanitize(trim),
    validate(len_char_min = 2, len_char_max = 35),
    derive(
        Debug,
        Clone,
        Display,
        Hash,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Serialize,
        Deserialize,
        AsRef
    )
)]
pub struct RegionName(String);

/// parse an url encoded string into a RegionName
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn url_region_name_parser<'src>()
-> impl Parser<'src, &'src str, RegionName, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    url_text_component_parser().try_map(|region_name, span| {
        RegionName::try_new(&region_name).map_err(|err| {
            chumsky::error::Rich::custom(
                span,
                format!("failed to parse url-encoded region name ({region_name}): {err:?}"),
            )
        })
    })
}

/// parse a string into a RegionName
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn region_name_parser<'src>()
-> impl Parser<'src, &'src str, RegionName, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    any()
        .filter(|c: &char| {
            c.is_alphabetic() || c.is_numeric() || *c == ' ' || *c == '\'' || *c == '-'
        })
        .repeated()
        .at_least(2)
        .collect::<String>()
        .try_map(|region_name, span| {
            RegionName::try_new(&region_name).map_err(|err| {
                chumsky::error::Rich::custom(
                    span,
                    format!("failed to parse region name ({region_name}): {err:?}"),
                )
            })
        })
}

/// A location inside Second Life the way it is usually represented in
/// SLURLs or map URLs, based on a Region Name and integer coordinates
/// inside the region
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Location {
    /// the name of the region of the location
    pub region_name: RegionName,
    /// the x coordinate inside the region
    pub x: u8,
    /// the y coordinate inside the region
    pub y: u8,
    /// the z coordinate inside the region
    pub z: u16,
}

/// parse a string into a Location where no component is url-encoded
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn location_parser<'src>()
-> impl Parser<'src, &'src str, Location, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    region_name_parser()
        .then_ignore(just('/'))
        .then(u8_parser())
        .then_ignore(just('/'))
        .then(u8_parser())
        .then_ignore(just('/'))
        .then(u16_parser())
        .map(|(((region_name, x), y), z)| Location::new(region_name, x, y, z))
}

/// parse a string into a Location where the region name is url encoded
/// but each component of the location is separated by an actual slash
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn url_location_parser<'src>()
-> impl Parser<'src, &'src str, Location, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    url_region_name_parser()
        .then_ignore(just('/'))
        .then(u8_parser())
        .then_ignore(just('/'))
        .then(u8_parser())
        .then_ignore(just('/'))
        .then(u16_parser())
        .map(|(((region_name, x), y), z)| Location::new(region_name, x, y, z))
}

/// parse a string into a Location from a URL-encoded location (the slashes in
/// particular)
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn url_encoded_location_parser<'src>()
-> impl Parser<'src, &'src str, Location, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    url_text_component_parser().try_map(|s, span| {
        location_parser().parse(&s).into_result().map_err(|err| {
            chumsky::error::Rich::custom(
                span,
                format!("Parsing {s} as location failed with: {err:#?}"),
            )
        })
    })
}

/// the possible errors that can occur when parsing a String to a `Location`
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, strum::EnumIs)]
pub enum LocationParseError {
    /// unexpected number of /-separated components in the location URL
    #[error(
        "unexpected number of /-separated components in the location URL {0}, found {1} expected 4 (for a bare location) or 8 (for a URL)"
    )]
    UnexpectedComponentCount(String, usize),
    /// unexpected scheme in the location URL
    #[error("unexpected scheme in the location URL {0}, found {1}, expected http: or https:")]
    UnexpectedScheme(String, String),
    /// unexpected non-empty second component in location URL
    #[error(
        "unexpected non-empty second component in location URL {0}, found {1}, expected http or https"
    )]
    UnexpectedNonEmptySecondComponent(String, String),
    /// unexpected host in the location URL
    #[error(
        "unexpected host in the location URL {0}, found {1}, expected maps.secondlife.com or slurl.com"
    )]
    UnexpectedHost(String, String),
    /// unexpected path in the location URL
    #[error("unexpected path in the location URL {0}, found {1}, expected secondlife")]
    UnexpectedPath(String, String),
    /// error parsing the region name
    #[error("error parsing the region name {0}: {1}")]
    RegionName(String, RegionNameError),
    /// error parsing the X coordinate
    #[error("error parsing the X coordinate {0}: {1}")]
    X(String, std::num::ParseIntError),
    /// error parsing the Y coordinate
    #[error("error parsing the Y coordinate {0}: {1}")]
    Y(String, std::num::ParseIntError),
    /// error parsing the Z coordinate
    #[error("error parsing the Z coordinate {0}: {1}")]
    Z(String, std::num::ParseIntError),
}

impl std::str::FromStr for Location {
    type Err = LocationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // if the string is an USB-notecard line drop everything after the first comma
        let usb_location = s
            .split_once(',')
            .map_or(s, |(usb_location, _usb_comment)| usb_location);
        let parts = usb_location.split('/').collect::<Vec<_>>();
        if let [region_name, x, y, z] = parts.as_slice() {
            let region_name = RegionName::try_new(region_name.replace("%20", " "))
                .map_err(|err| LocationParseError::RegionName(s.to_owned(), err))?;
            let x = x
                .parse()
                .map_err(|err| LocationParseError::X(s.to_owned(), err))?;
            let y = y
                .parse()
                .map_err(|err| LocationParseError::Y(s.to_owned(), err))?;
            let z = z
                .parse()
                .map_err(|err| LocationParseError::Z(s.to_owned(), err))?;
            return Ok(Self {
                region_name,
                x,
                y,
                z,
            });
        }
        if let [scheme, second_component, host, path, region_name, x, y, z] = parts.as_slice() {
            if *scheme != "http:" && *scheme != "https:" {
                return Err(LocationParseError::UnexpectedScheme(
                    s.to_owned(),
                    scheme.to_string(),
                ));
            }
            if !second_component.is_empty() {
                return Err(LocationParseError::UnexpectedNonEmptySecondComponent(
                    s.to_owned(),
                    second_component.to_string(),
                ));
            }
            if *host != "maps.secondlife.com" && *host != "slurl.com" {
                return Err(LocationParseError::UnexpectedHost(
                    s.to_owned(),
                    host.to_string(),
                ));
            }
            if *path != "secondlife" {
                return Err(LocationParseError::UnexpectedPath(
                    s.to_owned(),
                    path.to_string(),
                ));
            }
            let region_name = RegionName::try_new(region_name.replace("%20", " "))
                .map_err(|err| LocationParseError::RegionName(s.to_owned(), err))?;
            let x = x
                .parse()
                .map_err(|err| LocationParseError::X(s.to_owned(), err))?;
            let y = y
                .parse()
                .map_err(|err| LocationParseError::Y(s.to_owned(), err))?;
            let z = z
                .parse()
                .map_err(|err| LocationParseError::Z(s.to_owned(), err))?;
            return Ok(Self {
                region_name,
                x,
                y,
                z,
            });
        }
        Err(LocationParseError::UnexpectedComponentCount(
            s.to_owned(),
            parts.len(),
        ))
    }
}

impl Location {
    /// Creates a new `Location`
    #[must_use]
    pub const fn new(region_name: RegionName, x: u8, y: u8, z: u16) -> Self {
        Self {
            region_name,
            x,
            y,
            z,
        }
    }

    /// The region name of this `Location`
    #[must_use]
    pub const fn region_name(&self) -> &RegionName {
        &self.region_name
    }

    /// The x coordinate of the `Location`
    #[must_use]
    pub const fn x(&self) -> u8 {
        self.x
    }

    /// The y coordinate of the `Location`
    #[must_use]
    pub const fn y(&self) -> u8 {
        self.y
    }

    /// The z coordinate of the `Location`
    #[must_use]
    pub const fn z(&self) -> u16 {
        self.z
    }

    /// returns a maps.secondlife.com URL for the `Location`
    #[must_use]
    pub fn as_maps_url(&self) -> String {
        format!(
            "https://maps.secondlife.com/secondlife/{}/{}/{}/{}",
            self.region_name, self.x, self.y, self.z
        )
    }
}

/// A location inside Second Life the way it is usually represented in
/// SLURLs or map URLs, based on a Region Name and integer coordinates
/// inside the region, this variant allows out of bounds coordinates
/// (negative and 256 or above for x and y and negative for z)
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UnconstrainedLocation {
    /// the name of the region of the location
    pub region_name: RegionName,
    /// the x coordinate inside the region
    pub x: i16,
    /// the y coordinate inside the region
    pub y: i16,
    /// the z coordinate inside the region
    pub z: i32,
}

impl UnconstrainedLocation {
    /// Creates a new `UnconstrainedLocation`
    #[must_use]
    pub const fn new(region_name: RegionName, x: i16, y: i16, z: i32) -> Self {
        Self {
            region_name,
            x,
            y,
            z,
        }
    }

    /// The region name of this `UnconstrainedLocation`
    #[must_use]
    pub const fn region_name(&self) -> &RegionName {
        &self.region_name
    }

    /// The x coordinate of the `UnconstrainedLocation`
    #[must_use]
    pub const fn x(&self) -> i16 {
        self.x
    }

    /// The y coordinate of the `UnconstrainedLocation`
    #[must_use]
    pub const fn y(&self) -> i16 {
        self.y
    }

    /// The z coordinate of the `UnconstrainedLocation`
    #[must_use]
    pub const fn z(&self) -> i32 {
        self.z
    }
}

/// parse a string into an UnconstrainedLocation where nothing is urlencoded
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn unconstrained_location_parser<'src>() -> impl Parser<
    'src,
    &'src str,
    UnconstrainedLocation,
    chumsky::extra::Err<chumsky::error::Rich<'src, char>>,
> {
    region_name_parser()
        .then_ignore(just('/'))
        .then(i16_parser())
        .then_ignore(just('/'))
        .then(i16_parser())
        .then_ignore(just('/'))
        .then(i32_parser())
        .map(|(((region_name, x), y), z)| UnconstrainedLocation::new(region_name, x, y, z))
}

/// parse a string into an UnconstrainedLocation where the region is urlencoded
/// but the components are separated by actual slashes
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn url_unconstrained_location_parser<'src>() -> impl Parser<
    'src,
    &'src str,
    UnconstrainedLocation,
    chumsky::extra::Err<chumsky::error::Rich<'src, char>>,
> {
    url_region_name_parser()
        .then_ignore(just('/'))
        .then(i16_parser())
        .then_ignore(just('/'))
        .then(i16_parser())
        .then_ignore(just('/'))
        .then(i32_parser())
        .map(|(((region_name, x), y), z)| UnconstrainedLocation::new(region_name, x, y, z))
}

/// parse a string into an UnconstrainedLocation where the entire location is
/// urlencoded with urlencoded slashes
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn urlencoded_unconstrained_location_parser<'src>() -> impl Parser<
    'src,
    &'src str,
    UnconstrainedLocation,
    chumsky::extra::Err<chumsky::error::Rich<'src, char>>,
> {
    url_region_name_parser()
        .then_ignore(just('/'))
        .then(i16_parser())
        .then_ignore(just('/'))
        .then(i16_parser())
        .then_ignore(just('/'))
        .then(i32_parser())
        .map(|(((region_name, x), y), z)| UnconstrainedLocation::new(region_name, x, y, z))
}

impl TryFrom<UnconstrainedLocation> for Location {
    type Error = std::num::TryFromIntError;

    fn try_from(value: UnconstrainedLocation) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.region_name,
            value.x.try_into()?,
            value.y.try_into()?,
            value.z.try_into()?,
        ))
    }
}

impl From<Location> for UnconstrainedLocation {
    fn from(value: Location) -> Self {
        Self {
            region_name: value.region_name,
            x: value.x.into(),
            y: value.y.into(),
            z: value.z.into(),
        }
    }
}

/// The map tile zoom level for the Second Life main map
#[nutype::nutype(
    validate(greater_or_equal = 1, less_or_equal = 8),
    derive(
        Debug,
        Clone,
        Copy,
        Display,
        FromStr,
        Hash,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Serialize,
        Deserialize
    )
)]
pub struct ZoomLevel(u8);

/// Errors that can occur when trying to find the correct zoom level to fit
/// regions into an output image of a given size
#[derive(Debug, Clone, thiserror::Error, strum::EnumIs)]
pub enum ZoomFitError {
    /// The region size in the x direction can not be zero
    #[error("region size in x direction can not be zero")]
    RegionSizeXZero,

    /// The region size in the y direction can not be zero
    #[error("region size in y direction can not be zero")]
    RegionSizeYZero,

    /// The output image size in the x direction can not be zero
    #[error("output image size in x direction can not be zero")]
    OutputSizeXZero,

    /// The output image size in the y direction can not be zero
    #[error("output image size in y direction can not be zero")]
    OutputSizeYZero,

    /// Error converting a logarithm value into a `u8` (should never happen)
    #[error("error converting a logarithm value into a u8")]
    LogarithmConversionError(#[from] std::num::TryFromIntError),

    /// Error creating the zoom level from the calculated value
    /// (should never happen)
    #[error("error creating zoom level from calculated value")]
    ZoomLevelError(#[from] ZoomLevelError),
}

impl ZoomLevel {
    /// returns the map tile size in number of regions at this zoom level
    ///
    /// This applies to both dimensions equally since both regions and map tiles
    /// are square
    #[must_use]
    pub fn tile_size(&self) -> u16 {
        let exponent: u32 = self.into_inner().into();
        let exponent = exponent.saturating_sub(1);
        2u16.pow(exponent)
    }

    /// returns the map tile size in pixels at this zoom level
    ///
    /// This applies to both dimensions equally since both regions and map tiles
    /// are square
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "both values we multiply here are u16 originally so their product should never overflow an u32"
    )]
    #[must_use]
    pub fn tile_size_in_pixels(&self) -> u32 {
        let tile_size: u32 = self.tile_size().into();
        let region_size_in_map_tile_in_pixels: u32 = self.pixels_per_region().into();
        tile_size * region_size_in_map_tile_in_pixels
    }

    /// returns the lower left (lowest coordinate for each axis) coordinate of
    /// the map tile containing the given grid coordinates at this zoom level
    ///
    /// That is the coordinates used for the file name of the map tile at this
    /// zoom level that contains the region (or gap where a region could be)
    /// given by the grid coordinates
    #[must_use]
    pub fn map_tile_corner(&self, GridCoordinates { x, y }: &GridCoordinates) -> GridCoordinates {
        let tile_size = u32::from(self.tile_size());
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "remainder should not have any side-effects since tile_size is never 0 (no division by zero issues) or negative (no issues with x or y being e.g. i16::MIN which overflows when the sign is flipped)"
        )]
        GridCoordinates {
            x: x.saturating_sub(x % tile_size),
            y: y.saturating_sub(y % tile_size),
        }
    }

    /// returns the size of a region in pixels in a map tile of this zoom level
    ///
    /// The size applies to both dimensions equally since both regions and map tiles
    /// are square
    #[must_use]
    pub fn pixels_per_region(&self) -> u16 {
        let exponent: u32 = self.into_inner().into();
        let exponent = exponent.saturating_sub(1);
        let exponent = 8u32.saturating_sub(exponent);
        2u16.pow(exponent)
    }

    /// returns the number of pixels per meter at this zoom level
    #[must_use]
    pub fn pixels_per_meter(&self) -> f32 {
        f32::from(self.pixels_per_region()) / 256f32
    }

    /// returns the zoom level that is the highest zoom level that makes sense
    /// to use if we want to fit a given area of regions into a given image size
    /// assuming we want to always have one map tile pixel on one output pixel
    ///
    /// # Errors
    ///
    /// returns an error if any of the parameters are zero or in the (theoretically
    /// impossible if the algorithm is correct) case that ZoomLevel::try_new()
    /// returns an error on the calculated value
    pub fn max_zoom_level_to_fit_regions_into_output_image(
        region_x: u32,
        region_y: u32,
        output_x: u32,
        output_y: u32,
    ) -> Result<Self, ZoomFitError> {
        if region_x == 0 {
            return Err(ZoomFitError::RegionSizeXZero);
        }
        if region_y == 0 {
            return Err(ZoomFitError::RegionSizeYZero);
        }
        if output_x == 0 {
            return Err(ZoomFitError::OutputSizeXZero);
        }
        if output_y == 0 {
            return Err(ZoomFitError::OutputSizeYZero);
        }
        let output_pixels_per_region_x: u32 = output_x.div_ceil(region_x);
        let output_pixels_per_region_y: u32 = output_y.div_ceil(region_y);
        let max_zoom_level_x: u8 = 9u8.saturating_sub(std::cmp::min(
            8,
            output_pixels_per_region_x
                .ilog2()
                .try_into()
                .map_err(ZoomFitError::LogarithmConversionError)?,
        ));
        let max_zoom_level_y: u8 = 9u8.saturating_sub(std::cmp::min(
            8,
            output_pixels_per_region_y
                .ilog2()
                .try_into()
                .map_err(ZoomFitError::LogarithmConversionError)?,
        ));
        Ok(Self::try_new(std::cmp::max(
            max_zoom_level_x,
            max_zoom_level_y,
        ))?)
    }
}

/// describes a map tile
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is used outside this module"
)]
pub struct MapTileDescriptor {
    /// the zoom level of the map tile
    zoom_level: ZoomLevel,
    /// the lower left corner of the map tile
    lower_left_corner: GridCoordinates,
}

impl MapTileDescriptor {
    /// create a new `MapTileDescriptor`
    ///
    /// this will automatically normalize the given `GridCoordinates` to the
    /// lower left corner of a map tile at that zoom level
    #[must_use]
    pub fn new(zoom_level: ZoomLevel, grid_coordinates: GridCoordinates) -> Self {
        let lower_left_corner = zoom_level.map_tile_corner(&grid_coordinates);
        Self {
            zoom_level,
            lower_left_corner,
        }
    }

    /// the `ZoomLevel` of the map tile
    #[must_use]
    pub const fn zoom_level(&self) -> &ZoomLevel {
        &self.zoom_level
    }

    /// the `GridCoordinates` of the lower left corner of this map tile
    #[must_use]
    pub const fn lower_left_corner(&self) -> &GridCoordinates {
        &self.lower_left_corner
    }

    /// the size of this map tile in regions
    #[must_use]
    pub fn tile_size(&self) -> u16 {
        self.zoom_level.tile_size()
    }

    /// the size of this map tile in pixels
    #[must_use]
    pub fn tile_size_in_pixels(&self) -> u32 {
        self.zoom_level.tile_size_in_pixels()
    }

    /// the grid rectangle covered by this map tile
    #[must_use]
    pub fn grid_rectangle(&self) -> GridRectangle {
        GridRectangle::new(
            self.lower_left_corner,
            GridCoordinates::new(
                self.lower_left_corner
                    .x()
                    .saturating_add(u32::from(self.zoom_level.tile_size()))
                    .saturating_sub(1),
                self.lower_left_corner
                    .y()
                    .saturating_add(u32::from(self.zoom_level.tile_size()))
                    .saturating_sub(1),
            ),
        )
    }
}

/// A waypoint in the Universal Sailor Buddy (USB) notecard format
#[derive(Debug, Clone)]
pub struct USBWaypoint {
    /// the location of the waypoint
    location: Location,
    /// the comment for the waypoint if any
    comment: Option<String>,
}

impl USBWaypoint {
    /// Create a new USB waypoint
    #[must_use]
    pub const fn new(location: Location, comment: Option<String>) -> Self {
        Self { location, comment }
    }

    /// get the location of the waypoint
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// get the region coordinates of the waypoint
    #[must_use]
    pub fn region_coordinates(&self) -> RegionCoordinates {
        RegionCoordinates::new(
            f32::from(self.location.x()),
            f32::from(self.location.y()),
            f32::from(self.location.z()),
        )
    }

    /// get the comment for the waypoint if any
    #[must_use]
    pub const fn comment(&self) -> Option<&String> {
        self.comment.as_ref()
    }
}

impl std::fmt::Display for USBWaypoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.location.as_maps_url())?;
        if let Some(comment) = &self.comment {
            write!(f, ",{comment}")?;
        }
        Ok(())
    }
}

impl std::str::FromStr for USBWaypoint {
    type Err = LocationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((location, comment)) = s.split_once(',') {
            Ok(Self {
                location: location.parse()?,
                comment: Some(comment.to_owned()),
            })
        } else {
            Ok(Self {
                location: s.parse()?,
                comment: None,
            })
        }
    }
}

/// An Universal Sailor Buddy (USB) notecard
#[derive(Debug, Clone)]
pub struct USBNotecard {
    /// the waypoints in the notecard
    waypoints: Vec<USBWaypoint>,
}

/// Errors that can happen when an USB notecard is read from a file
#[derive(Debug, thiserror::Error, strum::EnumIs)]
pub enum USBNotecardLoadError {
    /// I/O errors opening or reading the file
    #[error("I/O error opening or reading the file: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error deserializing the USB notecard lines
    #[error("parse error deserializing the USB notecard lines: {0}")]
    LocationParseError(#[from] LocationParseError),
}

impl USBNotecard {
    /// Create a new USB notecard
    #[must_use]
    pub const fn new(waypoints: Vec<USBWaypoint>) -> Self {
        Self { waypoints }
    }

    /// get the waypoints in the notecard
    #[must_use]
    pub fn waypoints(&self) -> &[USBWaypoint] {
        &self.waypoints
    }

    /// load an USB Notecard from a text file
    ///
    /// # Errors
    ///
    /// this returns an error if either reading the file or parsing the content
    /// as a `USBNotecard` fail
    pub fn load_from_file(filename: &std::path::Path) -> Result<Self, USBNotecardLoadError> {
        let contents = std::fs::read_to_string(filename)?;
        Ok(contents.parse()?)
    }
}

impl std::fmt::Display for USBNotecard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for waypoint in &self.waypoints {
            writeln!(f, "{waypoint}")?;
        }
        Ok(())
    }
}

impl std::str::FromStr for USBNotecard {
    type Err = LocationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.lines()
            .map(|line| line.parse::<USBWaypoint>())
            .collect::<Result<Vec<_>, _>>()
            .map(|waypoints| Self { waypoints })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_location_bare() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            "Beach%20Valley/110/67/24".parse::<Location>(),
            Ok(Location {
                region_name: RegionName::try_new("Beach Valley")?,
                x: 110,
                y: 67,
                z: 24
            }),
        );
        Ok(())
    }

    #[test]
    fn test_parse_location_url_maps() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            "http://maps.secondlife.com/secondlife/Beach%20Valley/110/67/24".parse::<Location>(),
            Ok(Location {
                region_name: RegionName::try_new("Beach Valley")?,
                x: 110,
                y: 67,
                z: 24
            }),
        );
        Ok(())
    }

    #[test]
    fn test_parse_location_url_slurl() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            "http://slurl.com/secondlife/Beach%20Valley/110/67/24".parse::<Location>(),
            Ok(Location {
                region_name: RegionName::try_new("Beach Valley")?,
                x: 110,
                y: 67,
                z: 24
            }),
        );
        Ok(())
    }

    #[test]
    fn test_parse_location_bare_with_usb_comment() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            "Beach%20Valley/110/67/24,MUSTER".parse::<Location>(),
            Ok(Location {
                region_name: RegionName::try_new("Beach Valley")?,
                x: 110,
                y: 67,
                z: 24
            }),
        );
        Ok(())
    }

    #[test]
    fn test_grid_rectangle_intersection_upper_right_corner()
    -> Result<(), Box<dyn std::error::Error>> {
        let rect1 = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        let rect2 = GridRectangle::new(GridCoordinates::new(15, 15), GridCoordinates::new(25, 25));
        assert_eq!(
            rect1.intersect(&rect2),
            Some(GridRectangle::new(
                GridCoordinates::new(15, 15),
                GridCoordinates::new(20, 20),
            ))
        );
        Ok(())
    }

    #[test]
    fn test_grid_rectangle_intersection_upper_left_corner() -> Result<(), Box<dyn std::error::Error>>
    {
        let rect1 = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        let rect2 = GridRectangle::new(GridCoordinates::new(5, 15), GridCoordinates::new(15, 25));
        assert_eq!(
            rect1.intersect(&rect2),
            Some(GridRectangle::new(
                GridCoordinates::new(10, 15),
                GridCoordinates::new(15, 20),
            ))
        );
        Ok(())
    }

    #[test]
    fn test_grid_rectangle_intersection_lower_left_corner() -> Result<(), Box<dyn std::error::Error>>
    {
        let rect1 = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        let rect2 = GridRectangle::new(GridCoordinates::new(5, 5), GridCoordinates::new(15, 15));
        assert_eq!(
            rect1.intersect(&rect2),
            Some(GridRectangle::new(
                GridCoordinates::new(10, 10),
                GridCoordinates::new(15, 15),
            ))
        );
        Ok(())
    }

    #[test]
    fn test_grid_rectangle_intersection_lower_right_corner()
    -> Result<(), Box<dyn std::error::Error>> {
        let rect1 = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        let rect2 = GridRectangle::new(GridCoordinates::new(15, 5), GridCoordinates::new(25, 15));
        assert_eq!(
            rect1.intersect(&rect2),
            Some(GridRectangle::new(
                GridCoordinates::new(15, 10),
                GridCoordinates::new(20, 15),
            ))
        );
        Ok(())
    }

    #[test]
    fn test_grid_rectangle_intersection_no_overlap() -> Result<(), Box<dyn std::error::Error>> {
        let rect1 = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        let rect2 = GridRectangle::new(GridCoordinates::new(30, 30), GridCoordinates::new(40, 40));
        assert_eq!(rect1.intersect(&rect2), None);
        Ok(())
    }

    #[test]
    fn test_grid_rectangle_expanded_west() {
        let rect = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        assert_eq!(rect.expanded_west(0), rect);
        assert_eq!(
            rect.expanded_west(3),
            GridRectangle::new(GridCoordinates::new(7, 10), GridCoordinates::new(20, 20)),
        );
        let near_edge =
            GridRectangle::new(GridCoordinates::new(2, 10), GridCoordinates::new(20, 20));
        assert_eq!(
            near_edge.expanded_west(5),
            GridRectangle::new(GridCoordinates::new(0, 10), GridCoordinates::new(20, 20)),
        );
    }

    #[test]
    fn test_grid_rectangle_expanded_east() {
        let rect = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        assert_eq!(rect.expanded_east(0), rect);
        assert_eq!(
            rect.expanded_east(3),
            GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(23, 20)),
        );
        let near_edge = GridRectangle::new(
            GridCoordinates::new(10, 10),
            GridCoordinates::new(u32::MAX - 2, 20),
        );
        assert_eq!(
            near_edge.expanded_east(5),
            GridRectangle::new(
                GridCoordinates::new(10, 10),
                GridCoordinates::new(u32::MAX, 20),
            ),
        );
    }

    #[test]
    fn test_grid_rectangle_expanded_south() {
        let rect = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        assert_eq!(rect.expanded_south(0), rect);
        assert_eq!(
            rect.expanded_south(3),
            GridRectangle::new(GridCoordinates::new(10, 7), GridCoordinates::new(20, 20)),
        );
        let near_edge =
            GridRectangle::new(GridCoordinates::new(10, 2), GridCoordinates::new(20, 20));
        assert_eq!(
            near_edge.expanded_south(5),
            GridRectangle::new(GridCoordinates::new(10, 0), GridCoordinates::new(20, 20)),
        );
    }

    #[test]
    fn test_grid_rectangle_expanded_north() {
        let rect = GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 20));
        assert_eq!(rect.expanded_north(0), rect);
        assert_eq!(
            rect.expanded_north(3),
            GridRectangle::new(GridCoordinates::new(10, 10), GridCoordinates::new(20, 23)),
        );
        let near_edge = GridRectangle::new(
            GridCoordinates::new(10, 10),
            GridCoordinates::new(20, u32::MAX - 2),
        );
        assert_eq!(
            near_edge.expanded_north(5),
            GridRectangle::new(
                GridCoordinates::new(10, 10),
                GridCoordinates::new(20, u32::MAX),
            ),
        );
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_url_region_name_parser_no_whitespace() -> Result<(), Box<dyn std::error::Error>> {
        let region_name = "Viterbo";
        assert_eq!(
            url_region_name_parser().parse(region_name).into_result(),
            Ok(RegionName::try_new(region_name)?)
        );
        Ok(())
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_url_region_name_parser_url_whitespace() -> Result<(), Box<dyn std::error::Error>> {
        let region_name = "Da Boom";
        let input = region_name.replace(' ', "%20");
        assert_eq!(
            url_region_name_parser().parse(&input).into_result(),
            Ok(RegionName::try_new(region_name)?)
        );
        Ok(())
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_region_name_parser_whitespace() -> Result<(), Box<dyn std::error::Error>> {
        let region_name = "Da Boom";
        assert_eq!(
            region_name_parser().parse(region_name).into_result(),
            Ok(RegionName::try_new(region_name)?)
        );
        Ok(())
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_url_location_parser_no_whitespace() -> Result<(), Box<dyn std::error::Error>> {
        let region_name = "Viterbo";
        let input = format!("{region_name}/1/2/300");
        assert_eq!(
            url_location_parser().parse(&input).into_result(),
            Ok(Location {
                region_name: RegionName::try_new(region_name)?,
                x: 1,
                y: 2,
                z: 300
            })
        );
        Ok(())
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_url_location_parser_url_whitespace() -> Result<(), Box<dyn std::error::Error>> {
        let region_name = "Da Boom";
        let input = format!("{}/1/2/300", region_name.replace(' ', "%20"));
        assert_eq!(
            url_location_parser().parse(&input).into_result(),
            Ok(Location {
                region_name: RegionName::try_new(region_name)?,
                x: 1,
                y: 2,
                z: 300
            })
        );
        Ok(())
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_url_location_parser_url_whitespace_single_digit_after_space()
    -> Result<(), Box<dyn std::error::Error>> {
        let region_name = "Foo Bar 3";
        let input = format!("{}/1/2/300", region_name.replace(' ', "%20"));
        assert_eq!(
            url_location_parser().parse(&input).into_result(),
            Ok(Location {
                region_name: RegionName::try_new(region_name)?,
                x: 1,
                y: 2,
                z: 300
            })
        );
        Ok(())
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_region_coordinates_parser() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            region_coordinates_parser()
                .parse("{ 63.0486, 45.2515, 1501.08 }")
                .into_result(),
            Ok(RegionCoordinates {
                x: 63.0486,
                y: 45.2515,
                z: 1501.08,
            })
        );
        Ok(())
    }
}
