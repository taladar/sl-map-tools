//! Map-related data types

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
    /// common values are between roughly 395 and 1358
    x: u16,
    /// the y coordinate of the region, this is basically the vertical
    /// position of the region on the map increasing from south to north
    ///
    /// common values are between roughly 479 and 1430
    y: u16,
}

impl GridCoordinates {
    /// Create a new `GridCoordinates`
    #[must_use]
    pub fn new(x: u16, y: u16) -> Self {
        GridCoordinates { x, y }
    }

    /// The x coordinate of the region
    #[must_use]
    pub fn x(&self) -> u16 {
        self.x
    }

    /// The y coordinate of the region
    #[must_use]
    pub fn y(&self) -> u16 {
        self.y
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
        GridRectangle {
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

    /// returns the lower left corner of the rectangle
    #[must_use]
    pub fn lower_left_corner(&self) -> &GridCoordinates {
        &self.lower_left_corner
    }

    /// returns the upper right corner of the rectangle
    #[must_use]
    pub fn upper_right_corner(&self) -> &GridCoordinates {
        &self.upper_right_corner
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
        let (xs, ys): (Vec<u16>, Vec<u16>) = self.iter().map(|gc| (gc.x(), gc.y())).unzip();
        // unwrap is okay in these cases because we checked above that the container is non-empty
        #[allow(clippy::unwrap_used)]
        let (min_x, max_x) = (xs.iter().min().unwrap(), xs.iter().max().unwrap());
        #[allow(clippy::unwrap_used)]
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

impl RegionCoordinates {
    /// Create a new `RegionCoordinates`
    #[must_use]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        RegionCoordinates { x, y, z }
    }

    /// The x coordinate inside the region
    #[must_use]
    pub fn x(&self) -> f32 {
        self.x
    }

    /// The y coordinate inside the region
    #[must_use]
    pub fn y(&self) -> f32 {
        self.y
    }

    /// The z coordinate inside the region
    #[must_use]
    pub fn z(&self) -> f32 {
        self.z
    }
}

/// The name of a region
#[nutype::nutype(
    sanitize(trim),
    validate(len_char_min = 3, len_char_max = 35),
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
        Deserialize
    )
)]
pub struct RegionName(String);

/// A location inside Second Life the way it is usually represented in
/// SLURLs or map URLs, based on a Region Name and integer coordinates
/// inside the region
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Location {
    /// the name of the region of the location
    region_name: RegionName,
    /// the x coordinate inside the region
    x: u8,
    /// the y coordinate inside the region
    y: u8,
    /// the z coordinate inside the region
    z: u16,
}

/// the possible errors that can occur when parsing a String to a `Location`
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocationParseError {
    /// unexpected number of /-separated components in the location URL
    #[error("unexpected number of /-separated components in the location URL {0}, found {1} expected 4 (for a bare location) or 8 (for a URL)")]
    UnexpectedComponentCount(String, usize),
    /// unexpected scheme in the location URL
    #[error("unexpected scheme in the location URL {0}, found {1}, expected http: or https:")]
    UnexpectedScheme(String, String),
    /// unexpected non-empty second component in location URL
    #[error("unexpected non-empty second component in location URL {0}, found {1}, expected http or https")]
    UnexpectedNonEmptySecondComponent(String, String),
    /// unexpected host in the location URL
    #[error("unexpected host in the location URL {0}, found {1}, expected maps.secondlife.com or slurl.com")]
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
        let usb_parts = s.split(',').collect::<Vec<_>>();
        let parts = usb_parts[0].split('/').collect::<Vec<_>>();
        if parts.len() == 4 {
            let region_name = RegionName::try_new(parts[0].replace("%20", " "))
                .map_err(|err| LocationParseError::RegionName(s.to_owned(), err))?;
            let x = parts[1]
                .parse()
                .map_err(|err| LocationParseError::X(s.to_owned(), err))?;
            let y = parts[2]
                .parse()
                .map_err(|err| LocationParseError::Y(s.to_owned(), err))?;
            let z = parts[3]
                .parse()
                .map_err(|err| LocationParseError::Z(s.to_owned(), err))?;
            return Ok(Location {
                region_name,
                x,
                y,
                z,
            });
        }
        if parts.len() == 8 {
            if parts[0] != "http:" && parts[0] != "https:" {
                return Err(LocationParseError::UnexpectedScheme(
                    s.to_owned(),
                    parts[0].to_owned(),
                ));
            }
            if !parts[1].is_empty() {
                return Err(LocationParseError::UnexpectedNonEmptySecondComponent(
                    s.to_owned(),
                    parts[1].to_owned(),
                ));
            }
            if parts[2] != "maps.secondlife.com" && parts[2] != "slurl.com" {
                return Err(LocationParseError::UnexpectedHost(
                    s.to_owned(),
                    parts[2].to_owned(),
                ));
            }
            if parts[3] != "secondlife" {
                return Err(LocationParseError::UnexpectedPath(
                    s.to_owned(),
                    parts[3].to_owned(),
                ));
            }
            let region_name = RegionName::try_new(parts[4].replace("%20", " "))
                .map_err(|err| LocationParseError::RegionName(s.to_owned(), err))?;
            let x = parts[5]
                .parse()
                .map_err(|err| LocationParseError::X(s.to_owned(), err))?;
            let y = parts[6]
                .parse()
                .map_err(|err| LocationParseError::Y(s.to_owned(), err))?;
            let z = parts[7]
                .parse()
                .map_err(|err| LocationParseError::Z(s.to_owned(), err))?;
            return Ok(Location {
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
    pub fn new(region_name: RegionName, x: u8, y: u8, z: u16) -> Self {
        Location {
            region_name,
            x,
            y,
            z,
        }
    }

    /// The region name of this `Location`
    #[must_use]
    pub fn region_name(&self) -> &RegionName {
        &self.region_name
    }

    /// The x coordinate of the `Location`
    #[must_use]
    pub fn x(&self) -> u8 {
        self.x
    }

    /// The y coordinate of the `Location`
    #[must_use]
    pub fn y(&self) -> u8 {
        self.y
    }

    /// The z coordinate of the `Location`
    #[must_use]
    pub fn z(&self) -> u16 {
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
#[derive(Debug, Clone, thiserror::Error)]
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
        let exponent = exponent - 1;
        2u16.pow(exponent)
    }

    /// returns the map tile size in pixels at this zoom level
    ///
    /// This applies to both dimensions equally since both regions and map tiles
    /// are square
    #[must_use]
    pub fn tile_size_in_pixels(&self) -> u32 {
        let tile_size: u32 = self.tile_size().into();
        let region_size_in_map_tile_in_pixels: u32 =
            self.region_size_in_map_tile_in_pixels().into();
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
        let tile_size = self.tile_size();
        GridCoordinates {
            x: x - (x % tile_size),
            y: y - (y % tile_size),
        }
    }

    /// returns the size of a region in pixels in a map tile of this zoom level
    ///
    /// The size applies to both dimensions equally since both regions and map tiles
    /// are square
    #[must_use]
    pub fn region_size_in_map_tile_in_pixels(&self) -> u16 {
        let exponent: u32 = self.into_inner().into();
        let exponent = exponent - 1;
        let exponent = 8 - exponent;
        2u16.pow(exponent)
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
    #[allow(clippy::missing_panics_doc)]
    pub fn max_zoom_level_to_fit_regions_into_output_image(
        region_x: u16,
        region_y: u16,
        output_x: u32,
        output_y: u32,
    ) -> Result<ZoomLevel, ZoomFitError> {
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
        let output_pixels_per_region_x: u32 = output_x.div_ceil(region_x.into());
        let output_pixels_per_region_y: u32 = output_y.div_ceil(region_y.into());
        #[allow(clippy::expect_used)]
        let max_zoom_level_x: u8 = 9 - std::cmp::min(
            8,
            output_pixels_per_region_x
                .ilog2()
                .try_into()
                .expect("Logarithm of a u32 should always fit in a u8"),
        );
        #[allow(clippy::expect_used)]
        let max_zoom_level_y: u8 = 9 - std::cmp::min(
            8,
            output_pixels_per_region_y
                .ilog2()
                .try_into()
                .expect("Logarithm of a u32 should always fit in a u8"),
        );
        Ok(ZoomLevel::try_new(std::cmp::min(
            max_zoom_level_x,
            max_zoom_level_y,
        ))?)
    }
}

/// describes a map tile
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        MapTileDescriptor {
            zoom_level,
            lower_left_corner,
        }
    }

    /// the `ZoomLevel` of the map tile
    #[must_use]
    pub fn zoom_level(&self) -> &ZoomLevel {
        &self.zoom_level
    }

    /// the `GridCoordinates` of the lower left corner of this map tile
    #[must_use]
    pub fn lower_left_corner(&self) -> &GridCoordinates {
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
    pub fn new(location: Location, comment: Option<String>) -> Self {
        Self { location, comment }
    }

    /// get the location of the waypoint
    #[must_use]
    pub fn location(&self) -> &Location {
        &self.location
    }

    /// get the comment for the waypoint if any
    #[must_use]
    pub fn comment(&self) -> Option<&String> {
        self.comment.as_ref()
    }
}

impl std::fmt::Display for USBWaypoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.location.as_maps_url())?;
        if let Some(comment) = &self.comment {
            write!(f, ",{}", comment)?;
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
#[derive(Debug, thiserror::Error)]
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
    pub fn new(waypoints: Vec<USBWaypoint>) -> Self {
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
            writeln!(f, "{}", waypoint)?;
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
            .map(|waypoints| USBNotecard { waypoints })
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
}
