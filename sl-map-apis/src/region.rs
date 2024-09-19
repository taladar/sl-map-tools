//! Contains functionality related to converting region names to grid coordinates and vice versa
use sl_types::map::{GridCoordinates, RegionName, RegionNameError};

/// Represents the possible errors that can occur when converting a region name to grid coordinates
#[derive(Debug, thiserror::Error)]
pub enum RegionNameToGridCoordinatesError {
    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Error in response, probably means the region does not exist
    #[error("Error in response, probably means the region does not exist")]
    ResponseError,
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
pub async fn region_name_to_grid_coordinates(
    client: &reqwest::Client,
    region_name: &RegionName,
) -> Result<GridCoordinates, RegionNameToGridCoordinatesError> {
    let url = format!("https://cap.secondlife.com/cap/0/d661249b-2b5a-4436-966a-3d3b8d7a574f?var=coords&sim_name={}", region_name.to_string().replace(" ", "%20"));
    let response = client.get(&url).send().await?.text().await?;
    if response == "var coords = {'error' : true };" {
        return Err(RegionNameToGridCoordinatesError::ResponseError);
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
    if parts.len() != 2 {
        return Err(RegionNameToGridCoordinatesError::UnexpectedInfix(
            response.to_owned(),
        ));
    }
    let x = parts[0]
        .parse::<u16>()
        .map_err(|err| RegionNameToGridCoordinatesError::X(parts[0].to_owned(), err))?;
    let y = parts[1]
        .parse::<u16>()
        .map_err(|err| RegionNameToGridCoordinatesError::Y(parts[1].to_owned(), err))?;
    Ok(GridCoordinates::new(x, y))
}

/// Represents the possible errors that can occur when converting grid coordinates to a region name
#[derive(Debug, thiserror::Error)]
pub enum GridCoordinatesToRegionNameError {
    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Error in response, probably means the region does not exist
    #[error("Error in response, probably means the region does not exist")]
    ResponseError,
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
) -> Result<RegionName, GridCoordinatesToRegionNameError> {
    let url = format!("https://cap.secondlife.com/cap/0/b713fe80-283b-4585-af4d-a3b7d9a32492?var=region&grid_x={}&grid_y={}", grid_coordinates.x(), grid_coordinates.y());
    let response = client.get(&url).send().await?.text().await?;
    if response == "var region = {'error' : true };" {
        return Err(GridCoordinatesToRegionNameError::ResponseError);
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
    RegionName::try_new(response)
        .map_err(|err| GridCoordinatesToRegionNameError::RegionName(response.to_owned(), err))
}

/// a cache for region names to grid coordinates
/// that allows lookups in both directions
#[derive(Debug)]
pub struct RegionNameToGridCoordinatesCache {
    /// the reqwest Client used to lookup data not cached locally
    client: reqwest::Client,
    /// the cache database
    db: redb::Database,
    /// the cache ttl, after this we recheck with the server if a value has changed
    ttl: std::time::Duration,
}

/// describes an error that can occur as part of the cache operation for the `RegionNameToGridCoordinatesCache`
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
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

/// describes the redb table to store the last lookup of some grid coordinates
const GRID_COORDINATES_LAST_LOOKUP_TABLE: redb::TableDefinition<(u16, u16), u64> =
    redb::TableDefinition::new("last_grid_coordinate_lookup");

/// describes the redb table to store the last lookup of a region name
const REGION_NAME_LAST_LOOKUP_TABLE: redb::TableDefinition<String, u64> =
    redb::TableDefinition::new("last_region_name_lookup");

impl RegionNameToGridCoordinatesCache {
    /// create a new cache
    ///
    /// # Errors
    ///
    /// returns an error if the database could not be created or opened
    pub fn new(
        cache_directory: std::path::PathBuf,
        ttl: std::time::Duration,
    ) -> Result<Self, CacheError> {
        let client = reqwest::Client::new();
        let db = redb::Database::create(cache_directory.join("region_name.redb"))?;
        Ok(Self { client, db, ttl })
    }

    /// get the grid coordinates for a region name
    ///
    /// # Errors
    ///
    /// returns an error if either the local database operations or the HTTP requests fail
    pub async fn get_grid_coordinates(
        &self,
        region_name: &RegionName,
    ) -> Result<Option<GridCoordinates>, CacheError> {
        {
            let mut use_cache = false;
            let read_txn = self.db.begin_read()?;
            if let Ok(table) = read_txn.open_table(REGION_NAME_LAST_LOOKUP_TABLE) {
                if let Some(access_guard) = table.get(region_name.to_owned().into_inner())? {
                    if let Some(last_lookup_time) = std::time::UNIX_EPOCH
                        .checked_add(std::time::Duration::from_secs(access_guard.value()))
                    {
                        let now = std::time::SystemTime::now();
                        if now.duration_since(last_lookup_time)? < self.ttl {
                            use_cache = true;
                        }
                    }
                }
            }
            if use_cache {
                if let Ok(table) = read_txn.open_table(GRID_COORDINATE_CACHE_TABLE) {
                    if let Some(access_guard) = table.get(region_name.to_owned().into_inner())? {
                        let (x, y) = access_guard.value();
                        return Ok(Some(GridCoordinates::new(x, y)));
                    }
                    return Ok(None);
                }
            }
        }
        match region_name_to_grid_coordinates(&self.client, region_name).await {
            Ok(grid_coordinates) => {
                let write_txn = self.db.begin_write()?;
                let now = std::time::SystemTime::now();
                {
                    let mut table = write_txn.open_table(REGION_NAME_LAST_LOOKUP_TABLE)?;
                    table.insert(
                        region_name.to_owned().into_inner(),
                        now.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                    )?;
                }
                {
                    let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_TABLE)?;
                    table.insert(
                        region_name.to_owned().into_inner(),
                        (grid_coordinates.x(), grid_coordinates.y()),
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
                Ok(Some(grid_coordinates))
            }
            Err(RegionNameToGridCoordinatesError::ResponseError) => {
                let write_txn = self.db.begin_write()?;
                let now = std::time::SystemTime::now();
                {
                    let mut table = write_txn.open_table(REGION_NAME_LAST_LOOKUP_TABLE)?;
                    table.insert(
                        region_name.to_owned().into_inner(),
                        now.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                    )?;
                }
                {
                    let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_TABLE)?;
                    table.remove(region_name.to_owned().into_inner())?;
                }
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
        &self,
        grid_coordinates: &GridCoordinates,
    ) -> Result<Option<RegionName>, CacheError> {
        {
            let mut use_cache = false;
            let read_txn = self.db.begin_read()?;
            if let Ok(table) = read_txn.open_table(GRID_COORDINATES_LAST_LOOKUP_TABLE) {
                if let Some(access_guard) =
                    table.get((grid_coordinates.x(), grid_coordinates.y()))?
                {
                    if let Some(last_lookup_time) = std::time::UNIX_EPOCH
                        .checked_add(std::time::Duration::from_secs(access_guard.value()))
                    {
                        let now = std::time::SystemTime::now();
                        if now.duration_since(last_lookup_time)? < self.ttl {
                            use_cache = true;
                        }
                    }
                }
            }
            if use_cache {
                if let Ok(table) = read_txn.open_table(REGION_NAME_CACHE_TABLE) {
                    if let Some(access_guard) =
                        table.get((grid_coordinates.x(), grid_coordinates.y()))?
                    {
                        let region_name = access_guard.value();
                        return Ok(Some(RegionName::try_new(region_name)?));
                    }
                    return Ok(None);
                }
            }
        }
        match grid_coordinates_to_region_name(&self.client, grid_coordinates).await {
            Ok(region_name) => {
                let write_txn = self.db.begin_write()?;
                let now = std::time::SystemTime::now();
                {
                    let mut table = write_txn.open_table(GRID_COORDINATES_LAST_LOOKUP_TABLE)?;
                    table.insert(
                        (grid_coordinates.x(), grid_coordinates.y()),
                        now.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                    )?;
                }
                {
                    let mut table = write_txn.open_table(GRID_COORDINATE_CACHE_TABLE)?;
                    table.insert(
                        region_name.to_owned().into_inner(),
                        (grid_coordinates.x(), grid_coordinates.y()),
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
                Ok(Some(region_name))
            }
            Err(GridCoordinatesToRegionNameError::ResponseError) => {
                let write_txn = self.db.begin_write()?;
                let now = std::time::SystemTime::now();
                {
                    let mut table = write_txn.open_table(GRID_COORDINATES_LAST_LOOKUP_TABLE)?;
                    table.insert(
                        (grid_coordinates.x(), grid_coordinates.y()),
                        now.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                    )?;
                }
                {
                    let mut table = write_txn.open_table(REGION_NAME_CACHE_TABLE)?;
                    table.remove((grid_coordinates.x(), grid_coordinates.y()))?;
                }
                Ok(None)
            }
            Err(err) => Err(CacheError::RegionNameHttpError(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_region_name_to_grid_coordinates() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        assert_eq!(
            region_name_to_grid_coordinates(&client, &RegionName::try_new("Thorkell")?).await?,
            GridCoordinates::new(1136, 1075)
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_grid_coordinates_to_region_name() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        assert_eq!(
            grid_coordinates_to_region_name(&client, &GridCoordinates::new(1136, 1075)).await?,
            RegionName::try_new("Thorkell")?
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_region_name_to_grid_coordinates() -> Result<(), Box<dyn std::error::Error>>
    {
        let tempdir = tempfile::tempdir()?;
        let cache = RegionNameToGridCoordinatesCache::new(
            tempdir.path().to_path_buf(),
            std::time::Duration::from_secs(7 * 24 * 60 * 60),
        )?;
        assert_eq!(
            cache
                .get_grid_coordinates(&RegionName::try_new("Thorkell")?)
                .await?,
            Some(GridCoordinates::new(1136, 1075))
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_region_name_to_grid_coordinates_twice(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let cache = RegionNameToGridCoordinatesCache::new(
            tempdir.path().to_path_buf(),
            std::time::Duration::from_secs(7 * 24 * 60 * 60),
        )?;
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
    async fn test_cache_grid_coordinates_to_region_name() -> Result<(), Box<dyn std::error::Error>>
    {
        let tempdir = tempfile::tempdir()?;
        let cache = RegionNameToGridCoordinatesCache::new(
            tempdir.path().to_path_buf(),
            std::time::Duration::from_secs(7 * 24 * 60 * 60),
        )?;
        assert_eq!(
            cache
                .get_region_name(&GridCoordinates::new(1136, 1075))
                .await?,
            Some(RegionName::try_new("Thorkell")?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_grid_coordinates_to_region_name_twice(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let cache = RegionNameToGridCoordinatesCache::new(
            tempdir.path().to_path_buf(),
            std::time::Duration::from_secs(7 * 24 * 60 * 60),
        )?;
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
}
