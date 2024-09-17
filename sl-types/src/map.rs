//! Map-related data types

/// Grid coordinates for the position of a region on the map
///
/// the first region, Da Boom is located at 1000, 1000
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
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

/// Region coordinates for the position of something inside a region
///
/// Usually limited to 0..256 for x and y and 0..4096 for z (height)
/// but values outside those ranges are possible for positions of objects
/// in the process of crossing from one region to another or in similar
/// situations where they belong to one simulator logically but are located
/// outside of that simulator's region
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
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
    derive(Debug, Clone, Display, Hash, PartialEq, Eq, PartialOrd, Ord)
)]
pub struct RegionName(String);

/// A location inside Second Life the way it is usually represented in
/// SLURLs or map URLs, based on a Region Name and integer coordinates
/// inside the region
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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
        Debug, Clone, Copy, Display, FromStr, Hash, PartialEq, Eq, PartialOrd, Ord
    )
)]
pub struct ZoomLevel(u8);

impl ZoomLevel {
    /// returns the map tile size in number of regions (tiles are always square)
    /// at this zoom level
    #[must_use]
    pub fn tile_size(&self) -> u16 {
        let exponent: u32 = self.into_inner().into();
        let exponent = exponent - 1;
        2u16.pow(exponent)
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
