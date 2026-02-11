//! Contains functionality related to converting region names to grid coordinates and vice versa
use redb::ReadableDatabase as _;
use sl_types::map::{GridCoordinates, GridRectangle, RegionName, RegionNameError, USBNotecard};

/// Represents the possible errors that can occur when converting a region name to grid coordinates
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside the module"
)]
#[derive(Debug, thiserror::Error)]
pub enum RegionNameToGridCoordinatesError {
    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// failed to clone request for creation of cache policy
    #[error("failed to clone request for creation of cache policy")]
    FailedToCloneRequest,
    /// Unexpected prefix in response body
    #[error("Unexpected prefix in response body: {0}")]
    UnexpectedPrefix(String),
    /// Unexpected suffix in response body
    #[error("Unexpected suffix in response body: {0}")]
    UnexpectedSuffix(String),
    /// Unexpected infix in response body
    #[error("Unexpected infix in response body: {0}")]
    UnexpectedInfix(String),
    /// error parsing the X coordinate
    #[error("error parsing the X coordinate {0}: {1}")]
    X(String, std::num::ParseIntError),
    /// error parsing the Y coordinate
    #[error("error parsing the Y coordinate {0}: {1}")]
    Y(String, std::num::ParseIntError),
}

/// converts a `RegionName` to `GridCoordinates` using the Linden Lab API
///
/// # Errors
///
/// returns an error if the HTTP request fails or if the result couldn't
/// be parsed properly
#[expect(
    clippy::module_name_repetitions,
    reason = "the function is going to be used outside the module"
)]
pub async fn region_name_to_grid_coordinates(
    client: &reqwest::Client,
    region_name: &RegionName,
    cached_value_with_cache_policy: Option<(
        Option<GridCoordinates>,
        http_cache_semantics::CachePolicy,
    )>,
) -> Result<
    (Option<GridCoordinates>, http_cache_semantics::CachePolicy),
    RegionNameToGridCoordinatesError,
> {
    tracing::debug!(
        "Looking up grid coordinates for region name {}",
        region_name
    );
    let url = format!(
        "https://cap.secondlife.com/cap/0/d661249b-2b5a-4436-966a-3d3b8d7a574f?var=coords&sim_name={}",
        region_name.to_string().replace(' ', "%20")
    );
    let request = client.get(&url).build()?;
    if let Some((cached_value, cache_policy)) = cached_value_with_cache_policy {
        let now = std::time::SystemTime::now();
        if let http_cache_semantics::BeforeRequest::Fresh(_) =
            cache_policy.before_request(&request, now)
        {
            tracing::debug!("Using cached grid coordinates/absence");
            return Ok((cached_value, cache_policy));
        }
    }
    let response = client
        .execute(
            request
                .try_clone()
                .ok_or(RegionNameToGridCoordinatesError::FailedToCloneRequest)?,
        )
        .await?;
    let cache_policy = http_cache_semantics::CachePolicy::new(&request, &response);
    let response = response.text().await?;
    if response == "var coords = {'error' : true };" {
        tracing::debug!("Received negative response");
        return Ok((None, cache_policy));
    }
    let Some(response) = response.strip_prefix("var coords = {'x' : ") else {
        return Err(RegionNameToGridCoordinatesError::UnexpectedPrefix(
            response.to_owned(),
        ));
    };
    let Some(response) = response.strip_suffix(" };") else {
        return Err(RegionNameToGridCoordinatesError::UnexpectedSuffix(
            response.to_owned(),
        ));
    };
    let parts = response.split(", 'y' : ").collect::<Vec<_>>();
    let [x, y] = parts.as_slice() else {
        return Err(RegionNameToGridCoordinatesError::UnexpectedInfix(
            response.to_owned(),
        ));
    };
    let x = x
        .parse::<u16>()
        .map_err(|err| RegionNameToGridCoordinatesError::X(x.to_string(), err))?;
    let y = y
        .parse::<u16>()
        .map_err(|err| RegionNameToGridCoordinatesError::Y(y.to_string(), err))?;
    let grid_coordinates = GridCoordinates::new(x, y);
    tracing::debug!("Received response: {:?}", grid_coordinates);
    Ok((Some(grid_coordinates), cache_policy))
}

/// Represents the possible errors that can occur when converting grid coordinates to a region name
#[derive(Debug, thiserror::Error)]
pub enum GridCoordinatesToRegionNameError {
    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// failed to clone request for creation of cache policy
    #[error("failed to clone request for creation of cache policy")]
    FailedToCloneRequest,
    /// Unexpected prefix in response body
    #[error("Unexpected prefix in response body: {0}")]
    UnexpectedPrefix(String),
    /// Unexpected suffix in response body
    #[error("Unexpected suffix in response body: {0}")]
    UnexpectedSuffix(String),
    /// error parsing the region name
    #[error("error parsing the region name {0}: {1}")]
    RegionName(String, RegionNameError),
}

/// converts `GridCoordinates` to a `RegionName` using the Linden Lab API
///
/// # Errors
///
/// returns an error if the HTTP request fails or if the result couldn't
/// be parsed properly
pub async fn grid_coordinates_to_region_name(
    client: &reqwest::Client,
    grid_coordinates: &GridCoordinates,
    cached_value_with_cache_policy: Option<(Option<RegionName>, http_cache_semantics::CachePolicy)>,
) -> Result<(Option<RegionName>, http_cache_semantics::CachePolicy), GridCoordinatesToRegionNameError>
{
    tracing::debug!(
        "Looking up region name for grid coordinates {:?}",
        grid_coordinates
    );
    let url = format!(
        "https://cap.secondlife.com/cap/0/b713fe80-283b-4585-af4d-a3b7d9a32492?var=region&grid_x={}&grid_y={}",
        grid_coordinates.x(),
        grid_coordinates.y()
    );
    let request = client.get(&url).build()?;
    if let Some((cached_value, cache_policy)) = cached_value_with_cache_policy {
        let now = std::time::SystemTime::now();
        if let http_cache_semantics::BeforeRequest::Fresh(_) =
            cache_policy.before_request(&request, now)
        {
            tracing::debug!("Returning cached region name/absence");
            return Ok((cached_value, cache_policy));
        }
    }
    let response = client
        .execute(
            request
                .try_clone()
                .ok_or(GridCoordinatesToRegionNameError::FailedToCloneRequest)?,
        )
        .await?;
    let cache_policy = http_cache_semantics::CachePolicy::new(&request, &response);
    let response = response.text().await?;
    if response == "var region = {'error' : true };" {
        tracing::debug!("Received negative response");
        return Ok((None, cache_policy));
    }
    let Some(response) = response.strip_prefix("var region='") else {
        return Err(GridCoordinatesToRegionNameError::UnexpectedPrefix(
            response.to_string(),
        ));
    };
    let Some(response) = response.strip_suffix("';") else {
        return Err(GridCoordinatesToRegionNameError::UnexpectedSuffix(
            response.to_string(),
        ));
    };
    let region_name = RegionName::try_new(response)
        .map_err(|err| GridCoordinatesToRegionNameError::RegionName(response.to_owned(), err))?;
    tracing::debug!("Received region name: {region_name}");
    Ok((Some(region_name), cache_policy))
}

/// a cache for region names to grid coordinates
/// that allows lookups in both directions
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside the module"
)]
#[derive(Debug)]
pub struct RegionNameToGridCoordinatesCache {
    /// the reqwest Client used to lookup data not cached locally
    client: reqwest::Client,
    /// the cache database
    db: redb::Database,
    /// the in memory cache of region names to grid coordinates
    grid_coordinate_cache:
        lru::LruCache<RegionName, (Option<GridCoordinates>, http_cache_semantics::CachePolicy)>,
    /// the in memory cache of grid coordinates to region names
    region_name_cache:
        lru::LruCache<GridCoordinates, (Option<RegionName>, http_cache_semantics::CachePolicy)>,
}

/// describes an error that can occur as part of the cache operation for the `RegionNameToGridCoordinatesCache`
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// error decoding the JSON serialized CachePolicy
    #[error("error decoding the JSON serialized CachePolicy: {0}")]
    CachePolicyJsonDecodeError(#[from] serde_json::Error),
    /// redb database error
    #[error("redb database error: {0}")]
    DatabaseError(#[from] redb::DatabaseError),
    /// redb transaction error
    #[error("redb transaction error: {0}")]
    TransactionError(#[from] redb::TransactionError),
    /// redb table error
    #[error("redb table error: {0}")]
    TableError(#[from] redb::TableError),
    /// redb storage error
    #[error("redb storage error: {0}")]
    StorageError(#[from] redb::StorageError),
    /// redb commit error
    #[error("redb storage error: {0}")]
    CommitError(#[from] redb::CommitError),
    /// error looking up grid coordinates via HTTP
    #[error("error looking up grid coordinates via HTTP: {0}")]
    GridCoordinatesHttpError(#[from] RegionNameToGridCoordinatesError),
    /// error looking up region name via HTTP
    #[error("error looking up region name via HTTP: {0}")]
    RegionNameHttpError(#[from] GridCoordinatesToRegionNameError),
    /// error creating region name from cached string
    #[error("error creating region name from cached string: {0}")]
    RegionNameError(#[from] RegionNameError),
    /// error handling system time for cache age calculations
    #[error("error handling system time for cache age calculations: {0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),
}

/// describes the redb table to store region names and grid coordinates
const GRID_COORDINATE_CACHE_TABLE: redb::TableDefinition<String, (u16, u16)> =
    redb::TableDefinition::new("grid_coordinates");

/// describes the redb table to store grid coordinates and region names
const REGION_NAME_CACHE_TABLE: redb::TableDefinition<(u16, u16), String> =
    redb::TableDefinition::new("region_name");

/// describes the redb table to store the `http_cache_semantics::CachePolicy`
/// serialized as JSON for a region name to grid coordinate lookup
const GRID_COORDINATE_CACHE_POLICY_TABLE: redb::TableDefinition<String, String> =
    redb::TableDefinition::new("grid_coordinate_cache_policy");

/// describes the redb table to store the `http_cache_semantics::CachePolicy`
/// serialized as JSON for a grid coordinate to region name lookup
const REGION_NAME_CACHE_POLICY_TABLE: redb::TableDefinition<(u16, u16), String> =
    redb::TableDefinition::new("region_name_cache_policy");

impl RegionNameToGridCoordinatesCache {
    /// create a new cache
    ///
    /// # Errors
    ///
    /// returns an error if the database could not be created or opened
    pub fn new(cache_directory: std::path::PathBuf) -> Result<Self, CacheError> {
        let client = reqwest::Client::new();
        let db = redb::Database::create(cache_directory.join("region_name.redb"))?;
        let grid_coordinate_cache = lru::LruCache::unbounded();
        let region_name_cache = lru::LruCache::unbounded();
        Ok(Self {
            client,
            db,
            grid_coordinate_cache,
            region_name_cache,
        })
    }

    /// get the grid coordinates for a region name
    ///
    /// # Errors
    ///
    /// returns an error if either the local database operations or the HTTP requests fail
    pub async fn get_grid_coordinates(
        &mut self,
        region_name: &RegionName,
    ) -> Result<Option<GridCoordinates>, CacheError> {
        tracing::debug!("Retrieving grid coordinates for region {region_name:?}");
        let cached_value_with_cache_policy = {
            if let Some(memory_cached_value) = self.grid_coordinate_cache.get(region_name) {
                Some(memory_cached_value.to_owned())
            } else {
                let read_txn = self.db.begin_read()?;
                let cache_policy = {
                    if let Ok(table) = read_txn.open_table(GRID_COORDINATE_CACHE_POLICY_TABLE) {
                        if let Some(access_guard) =
                            table.get(region_name.to_owned().into_inner())?
                        {
                            let cache_policy: http_cache_semantics::CachePolicy =
                                serde_json::from_str(&access_guard.value())?;
                            Some(cache_policy)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(cache_policy) = cache_policy {
                    let cached_value = {
                        if let Ok(table) = read_txn.open_table(GRID_COORDINATE_CACHE_TABLE) {
                            if let Some(access_guard) =
                                table.get(region_name.to_owned().into_inner())?
                            {
                                let (x, y) = access_guard.value();
                                Some(GridCoordinates::new(x, y))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };
                    Some((cached_value, cache_policy))
                } else {
                    None
                }
            }
        };
        match region_name_to_grid_coordinates(
            &self.client,
            region_name,
            cached_value_with_cache_policy,
        )
        .await
        {
            Ok((Some(grid_coordinates), cache_policy)) => {
                if cache_policy.is_storable() {
                    tracing::debug!("Storing grid coordinates in cache");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_POLICY_TABLE)?;
                        table.insert(
                            region_name.to_owned().into_inner(),
                            serde_json::to_string(&cache_policy)?,
                        )?;
                    }
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_TABLE)?;
                        table.insert(
                            region_name.to_owned().into_inner(),
                            (grid_coordinates.x(), grid_coordinates.y()),
                        )?;
                    }
                    write_txn.commit()?;
                    self.grid_coordinate_cache.put(
                        region_name.to_owned(),
                        (Some(grid_coordinates), cache_policy),
                    );
                } else {
                    tracing::debug!("Grid coordinates are not storable");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_POLICY_TABLE)?;
                        table.remove(region_name.to_owned().into_inner())?;
                    }
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_TABLE)?;
                        table.remove(region_name.to_owned().into_inner())?;
                    }
                    write_txn.commit()?;
                    self.grid_coordinate_cache.pop(region_name);
                }
                tracing::debug!("Coordinates are {grid_coordinates:?}");
                Ok(Some(grid_coordinates))
            }
            Ok((None, cache_policy)) => {
                if cache_policy.is_storable() {
                    tracing::debug!("Storing negative response in cache");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_POLICY_TABLE)?;
                        table.insert(
                            region_name.to_owned().into_inner(),
                            serde_json::to_string(&cache_policy)?,
                        )?;
                    }
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_TABLE)?;
                        table.remove(region_name.to_owned().into_inner())?;
                    }
                    write_txn.commit()?;
                    self.grid_coordinate_cache
                        .put(region_name.to_owned(), (None, cache_policy));
                } else {
                    tracing::debug!("Negative response is not storable");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_POLICY_TABLE)?;
                        table.remove(region_name.to_owned().into_inner())?;
                    }
                    {
                        let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_TABLE)?;
                        table.remove(region_name.to_owned().into_inner())?;
                    }
                    write_txn.commit()?;
                    self.grid_coordinate_cache.pop(region_name);
                }
                tracing::debug!("No coordinates exist for that name");
                Ok(None)
            }
            Err(err) => Err(CacheError::GridCoordinatesHttpError(err)),
        }
    }

    /// get the region name for a set of grid coordinates
    ///
    /// # Errors
    ///
    /// returns an error if either the local database operations or the HTTP requests fail
    pub async fn get_region_name(
        &mut self,
        grid_coordinates: &GridCoordinates,
    ) -> Result<Option<RegionName>, CacheError> {
        tracing::debug!("Retrieving region name for grid coordinates {grid_coordinates:?}");
        let cached_value_with_cache_policy = {
            if let Some(memory_cached_value) = self.region_name_cache.get(grid_coordinates) {
                Some(memory_cached_value.to_owned())
            } else {
                let read_txn = self.db.begin_read()?;
                let cache_policy = {
                    if let Ok(table) = read_txn.open_table(REGION_NAME_CACHE_POLICY_TABLE) {
                        if let Some(access_guard) =
                            table.get((grid_coordinates.x(), grid_coordinates.y()))?
                        {
                            let cache_policy: http_cache_semantics::CachePolicy =
                                serde_json::from_str(&access_guard.value())?;
                            Some(cache_policy)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(cache_policy) = cache_policy {
                    let cached_value = {
                        if let Ok(table) = read_txn.open_table(REGION_NAME_CACHE_TABLE) {
                            if let Some(access_guard) =
                                table.get((grid_coordinates.x(), grid_coordinates.y()))?
                            {
                                let region_name = access_guard.value();
                                Some(RegionName::try_new(region_name)?)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };
                    Some((cached_value, cache_policy))
                } else {
                    None
                }
            }
        };
        match grid_coordinates_to_region_name(
            &self.client,
            grid_coordinates,
            cached_value_with_cache_policy,
        )
        .await
        {
            Ok((Some(region_name), cache_policy)) => {
                if cache_policy.is_storable() {
                    tracing::debug!("Storing region name in cache");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_POLICY_TABLE)?;
                        table.insert(
                            (grid_coordinates.x(), grid_coordinates.y()),
                            serde_json::to_string(&cache_policy)?,
                        )?;
                    }
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_TABLE)?;
                        table.insert(
                            (grid_coordinates.x(), grid_coordinates.y()),
                            region_name.to_owned().into_inner(),
                        )?;
                    }
                    write_txn.commit()?;
                    self.region_name_cache.put(
                        grid_coordinates.to_owned(),
                        (Some(region_name.to_owned()), cache_policy),
                    );
                } else {
                    tracing::warn!("Region name response is not storable");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_POLICY_TABLE)?;
                        table.remove((grid_coordinates.x(), grid_coordinates.y()))?;
                    }
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_TABLE)?;
                        table.remove((grid_coordinates.x(), grid_coordinates.y()))?;
                    }
                    write_txn.commit()?;
                    self.region_name_cache.pop(grid_coordinates);
                }
                tracing::debug!("Region name is {region_name:?}");
                Ok(Some(region_name))
            }
            Ok((None, cache_policy)) => {
                if cache_policy.is_storable() {
                    tracing::debug!("Storing negative response in cache");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_POLICY_TABLE)?;
                        table.insert(
                            (grid_coordinates.x(), grid_coordinates.y()),
                            serde_json::to_string(&cache_policy)?,
                        )?;
                    }
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_TABLE)?;
                        table.remove((grid_coordinates.x(), grid_coordinates.y()))?;
                    }
                    write_txn.commit()?;
                    self.region_name_cache
                        .put(grid_coordinates.to_owned(), (None, cache_policy));
                } else {
                    tracing::debug!("Negative response is not storable");
                    let write_txn = self.db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_POLICY_TABLE)?;
                        table.remove((grid_coordinates.x(), grid_coordinates.y()))?;
                    }
                    {
                        let mut table = write_txn.open_table(REGION_NAME_CACHE_TABLE)?;
                        table.remove((grid_coordinates.x(), grid_coordinates.y()))?;
                    }
                    write_txn.commit()?;
                    self.region_name_cache.pop(grid_coordinates);
                }
                tracing::debug!("No region name exists for those grid coordinates");
                Ok(None)
            }
            Err(err) => Err(CacheError::RegionNameHttpError(err)),
        }
    }
}

/// errors that can occur when converting a USB notecard to a grid rectangle
#[derive(Debug, thiserror::Error)]
pub enum USBNotecardToGridRectangleError {
    /// there were no waypoints in the USB notecards so we could not determine
    /// a grid rectangle for it
    #[error(
        "There were no waypoints in the USB notecards which made determining a grid rectangle for it impossible"
    )]
    NoUSBNotecardWaypoints,
    /// there were errors when converting region names to grid coordinates
    #[error("error converting region name to grid coordinates: {0}")]
    CacheError(#[from] CacheError),
    /// no grid coordinates were returned for one of the region names in the
    /// USB Notecard
    #[error("No grid coordinates were returned for one of the regions in the USB notecard: {0}")]
    NoGridCoordinatesForRegion(RegionName),
}

/// converts a USB notecard to the `GridRectangle` that contains all the waypoints
///
/// # Errors
///
/// returns an error if there were no waypoints or if conversions to grid coordinates failed
pub async fn usb_notecard_to_grid_rectangle(
    region_name_to_grid_coordinates_cache: &mut RegionNameToGridCoordinatesCache,
    usb_notecard: &USBNotecard,
) -> Result<GridRectangle, USBNotecardToGridRectangleError> {
    let mut lower_left_x = None;
    let mut lower_left_y = None;
    let mut upper_right_x = None;
    let mut upper_right_y = None;
    for waypoint in usb_notecard.waypoints() {
        let grid_coordinates = region_name_to_grid_coordinates_cache
            .get_grid_coordinates(waypoint.location().region_name())
            .await?;
        if let Some(grid_coordinates) = grid_coordinates {
            if let Some(llx) = lower_left_x {
                lower_left_x = Some(std::cmp::min(llx, grid_coordinates.x()));
            } else {
                lower_left_x = Some(grid_coordinates.x());
            }
            if let Some(lly) = lower_left_y {
                lower_left_y = Some(std::cmp::min(lly, grid_coordinates.y()));
            } else {
                lower_left_y = Some(grid_coordinates.y());
            }
            if let Some(urx) = upper_right_x {
                upper_right_x = Some(std::cmp::max(urx, grid_coordinates.x()));
            } else {
                upper_right_x = Some(grid_coordinates.x());
            }
            if let Some(ury) = upper_right_y {
                upper_right_y = Some(std::cmp::max(ury, grid_coordinates.y()));
            } else {
                upper_right_y = Some(grid_coordinates.y());
            }
        } else {
            return Err(USBNotecardToGridRectangleError::NoGridCoordinatesForRegion(
                waypoint.location().region_name().to_owned(),
            ));
        }
    }
    let Some(lower_left_x) = lower_left_x else {
        return Err(USBNotecardToGridRectangleError::NoUSBNotecardWaypoints);
    };
    let Some(lower_left_y) = lower_left_y else {
        return Err(USBNotecardToGridRectangleError::NoUSBNotecardWaypoints);
    };
    let Some(upper_right_x) = upper_right_x else {
        return Err(USBNotecardToGridRectangleError::NoUSBNotecardWaypoints);
    };
    let Some(upper_right_y) = upper_right_y else {
        return Err(USBNotecardToGridRectangleError::NoUSBNotecardWaypoints);
    };
    Ok(GridRectangle::new(
        GridCoordinates::new(lower_left_x, lower_left_y),
        GridCoordinates::new(upper_right_x, upper_right_y),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_region_name_to_grid_coordinates() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        assert_eq!(
            region_name_to_grid_coordinates(&client, &RegionName::try_new("Thorkell")?, None)
                .await?
                .0,
            Some(GridCoordinates::new(1136, 1075))
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_grid_coordinates_to_region_name() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        assert_eq!(
            grid_coordinates_to_region_name(&client, &GridCoordinates::new(1136, 1075), None)
                .await?
                .0,
            Some(RegionName::try_new("Thorkell")?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_region_name_to_grid_coordinates() -> Result<(), Box<dyn std::error::Error>>
    {
        let tempdir = tempfile::tempdir()?;
        let mut cache = RegionNameToGridCoordinatesCache::new(tempdir.path().to_path_buf())?;
        assert_eq!(
            cache
                .get_grid_coordinates(&RegionName::try_new("Thorkell")?)
                .await?,
            Some(GridCoordinates::new(1136, 1075))
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_region_name_to_grid_coordinates_twice()
    -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let mut cache = RegionNameToGridCoordinatesCache::new(tempdir.path().to_path_buf())?;
        assert_eq!(
            cache
                .get_grid_coordinates(&RegionName::try_new("Thorkell")?)
                .await?,
            Some(GridCoordinates::new(1136, 1075))
        );
        assert_eq!(
            cache
                .get_grid_coordinates(&RegionName::try_new("Thorkell")?)
                .await?,
            Some(GridCoordinates::new(1136, 1075))
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_region_name_to_grid_coordinates_negative_twice()
    -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let mut cache = RegionNameToGridCoordinatesCache::new(tempdir.path().to_path_buf())?;
        assert_eq!(
            cache
                .get_grid_coordinates(&RegionName::try_new("Thorkel")?)
                .await?,
            None,
        );
        assert_eq!(
            cache
                .get_grid_coordinates(&RegionName::try_new("Thorkel")?)
                .await?,
            None,
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_grid_coordinates_to_region_name() -> Result<(), Box<dyn std::error::Error>>
    {
        let tempdir = tempfile::tempdir()?;
        let mut cache = RegionNameToGridCoordinatesCache::new(tempdir.path().to_path_buf())?;
        assert_eq!(
            cache
                .get_region_name(&GridCoordinates::new(1136, 1075))
                .await?,
            Some(RegionName::try_new("Thorkell")?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_grid_coordinates_to_region_name_twice()
    -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let mut cache = RegionNameToGridCoordinatesCache::new(tempdir.path().to_path_buf())?;
        assert_eq!(
            cache
                .get_region_name(&GridCoordinates::new(1136, 1075))
                .await?,
            Some(RegionName::try_new("Thorkell")?)
        );
        assert_eq!(
            cache
                .get_region_name(&GridCoordinates::new(1136, 1075))
                .await?,
            Some(RegionName::try_new("Thorkell")?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_grid_coordinates_to_region_name_negative_twice()
    -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let mut cache = RegionNameToGridCoordinatesCache::new(tempdir.path().to_path_buf())?;
        assert_eq!(
            cache
                .get_region_name(&GridCoordinates::new(11136, 1075))
                .await?,
            None,
        );
        assert_eq!(
            cache
                .get_region_name(&GridCoordinates::new(11136, 1075))
                .await?,
            None,
        );
        Ok(())
    }
}
