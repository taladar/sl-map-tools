//! Contains functionality related to fetching map tiles
use std::path::PathBuf;

use sl_types::map::{GridCoordinates, GridRectangle, MapTileDescriptor, ZoomFitError, ZoomLevel};

/// represents a map tile fetched from the server
#[derive(Debug, Clone)]
pub struct MapTile {
    /// describes the map tile by lower left corner and zoom level
    descriptor: MapTileDescriptor,

    /// the actual image data
    image: image::DynamicImage,
}

impl MapTile {
    /// the descriptor of the map tile
    #[must_use]
    pub fn descriptor(&self) -> &MapTileDescriptor {
        &self.descriptor
    }

    /// the image data of the map tile
    #[must_use]
    pub fn image(&self) -> &image::DynamicImage {
        &self.image
    }
}

/// errors that can happen while fetching a map tile from the cache
#[derive(Debug, thiserror::Error)]
pub enum MapTileCacheError {
    /// error manipulating files in the cache directory
    #[error("error manipulating files in the cache directory: {0}")]
    CacheDirectoryFileError(std::io::Error),
    /// reqwest error when fetching the map tile from the server
    #[error("reqwest error when fetching the map tile from the server: {0}")]
    ReqwestError(#[from] reqwest::Error),
    /// HTTP request is not success
    #[error("HTTP request is not success: URL {0} response status {1} headers {2:#?} body {3}")]
    HttpError(
        String,
        reqwest::StatusCode,
        reqwest::header::HeaderMap,
        String,
    ),
    /// failed to clone request for cache policy use (which should not happen
    /// unless the body is a stream which it is not for us)
    #[error("failed to clone request for cache policy")]
    FailedToCloneRequest,
    /// error guessing image format
    #[error("error guessing image format: {0}")]
    ImageFormatGuessError(std::io::Error),
    /// error reading the raw map tile into an image
    #[error("error reading the raw map tile into an image: {0}")]
    ImageError(#[from] image::ImageError),
    /// error decoding the JSON serialized CachePolicy
    #[error("error decoding the JSON serialized CachePolicy: {0}")]
    CachePolicyJsonDecodeError(#[from] serde_json::Error),
}

/// a cache for map tiles on the local filesystem
#[derive(derive_more::Debug)]
pub struct MapTileCache {
    /// the client used to make HTTP requests for map tiles not in the local cache
    client: reqwest::Client,
    /// the rate limiter for map tile requests to the server
    #[debug(skip)]
    ratelimiter: Option<ratelimit::Ratelimiter>,
    /// the cache directory
    cache_directory: PathBuf,
}

impl MapTileCache {
    /// creates a new `MapTileCache`
    #[must_use]
    pub fn new(cache_directory: PathBuf, ratelimiter: Option<ratelimit::Ratelimiter>) -> Self {
        MapTileCache {
            client: reqwest::Client::new(),
            ratelimiter,
            cache_directory,
        }
    }

    /// the file name of a map tile cache file
    #[must_use]
    fn map_tile_file_name(&self, map_tile_descriptor: &MapTileDescriptor) -> String {
        format!(
            "map-{}-{}-{}-objects.jpg",
            map_tile_descriptor.zoom_level(),
            map_tile_descriptor.lower_left_corner().x(),
            map_tile_descriptor.lower_left_corner().y(),
        )
    }

    /// the file name of a map tile in the cache directory
    #[must_use]
    fn map_tile_cache_file_name(&self, map_tile_descriptor: &MapTileDescriptor) -> PathBuf {
        self.cache_directory
            .join(self.map_tile_file_name(map_tile_descriptor))
    }

    /// the file name of the cache policy file in the cache directory
    #[must_use]
    fn cache_policy_file_name(&self, map_tile_descriptor: &MapTileDescriptor) -> PathBuf {
        self.cache_directory.join(format!(
            "{}.cache-policy.json",
            self.map_tile_file_name(map_tile_descriptor)
        ))
    }

    /// the URL of a map tile on the Second Life main map server
    #[must_use]
    fn map_tile_url(&self, map_tile_descriptor: &MapTileDescriptor) -> String {
        format!(
            "https://secondlife-maps-cdn.akamaized.net/{}",
            self.map_tile_file_name(map_tile_descriptor),
        )
    }

    /// loads the cached `MapTile` and cache policy from the cache directory
    ///
    /// # Errors
    ///
    /// returns an error if file operations fail
    async fn fetch_cached_map_tile(
        &self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<Option<(MapTile, http_cache_semantics::CachePolicy)>, MapTileCacheError> {
        let cache_file = self.map_tile_cache_file_name(map_tile_descriptor);
        let cache_policy_file = self.cache_policy_file_name(map_tile_descriptor);
        if cache_file.exists() {
            if !cache_policy_file.exists() {
                std::fs::remove_file(cache_file)
                    .map_err(MapTileCacheError::CacheDirectoryFileError)?;
                Ok(None)
            } else {
                let cached_map_tile = image::ImageReader::open(cache_file)
                    .map_err(MapTileCacheError::CacheDirectoryFileError)?
                    .decode()?;
                let cache_policy = std::fs::read_to_string(cache_policy_file)
                    .map_err(MapTileCacheError::CacheDirectoryFileError)?;
                let cache_policy: http_cache_semantics::CachePolicy =
                    serde_json::from_str(&cache_policy)?;
                Ok(Some((
                    MapTile {
                        descriptor: map_tile_descriptor.to_owned(),
                        image: cached_map_tile,
                    },
                    cache_policy,
                )))
            }
        } else {
            if cache_policy_file.exists() {
                std::fs::remove_file(cache_policy_file)
                    .map_err(MapTileCacheError::CacheDirectoryFileError)?;
            }
            Ok(None)
        }
    }

    /// clears the data about a specific map tile from the cache
    async fn remove_cached_tile(
        &self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<(), MapTileCacheError> {
        let cache_file = self.map_tile_cache_file_name(map_tile_descriptor);
        let cache_policy_file = self.cache_policy_file_name(map_tile_descriptor);
        if cache_file.exists() {
            std::fs::remove_file(cache_file).map_err(MapTileCacheError::CacheDirectoryFileError)?;
        }
        if cache_policy_file.exists() {
            std::fs::remove_file(cache_policy_file)
                .map_err(MapTileCacheError::CacheDirectoryFileError)?;
        }
        Ok(())
    }

    /// fetches a map tile from the Second Life main map servers
    /// or the local cache
    ///
    /// # Errors
    ///
    /// returns an error if the HTTP request fails of if the result fails to be
    /// parsed as an image
    pub async fn get_map_tile(
        &self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<Option<MapTile>, MapTileCacheError> {
        let url = self.map_tile_url(map_tile_descriptor);
        let request = self.client.get(&url).build()?;
        let now = std::time::SystemTime::now();
        if let Some((cached_map_tile, cache_policy)) =
            self.fetch_cached_map_tile(map_tile_descriptor).await?
        {
            if let http_cache_semantics::BeforeRequest::Fresh(_) =
                cache_policy.before_request(&request, now)
            {
                return Ok(Some(cached_map_tile));
            }
            self.remove_cached_tile(map_tile_descriptor).await?;
        }
        if let Some(ratelimiter) = &self.ratelimiter {
            while let Err(duration) = ratelimiter.try_wait() {
                tokio::time::sleep(duration).await;
            }
        }
        let response = self
            .client
            .execute(
                request
                    .try_clone()
                    .ok_or(MapTileCacheError::FailedToCloneRequest)?,
            )
            .await?;
        if !response.status().is_success() {
            if response.status() == reqwest::StatusCode::FORBIDDEN {
                // FORBIDDEN (403) is returned when the file does not exist
                // which likely means there is no region/map tile
                return Ok(None);
            }
            return Err(MapTileCacheError::HttpError(
                url.to_owned(),
                response.status(),
                response.headers().to_owned(),
                response.text().await?,
            ));
        }
        let cache_policy = http_cache_semantics::CachePolicy::new(&request, &response);
        let raw_response_body = response.bytes().await?;
        let image = image::ImageReader::new(std::io::Cursor::new(raw_response_body))
            .with_guessed_format()
            .map_err(MapTileCacheError::ImageFormatGuessError)?
            .decode()?;
        if cache_policy.is_storable() {
            let cache_policy = serde_json::to_string(&cache_policy)?;
            std::fs::write(
                self.cache_policy_file_name(map_tile_descriptor),
                cache_policy,
            )
            .map_err(MapTileCacheError::CacheDirectoryFileError)?;
            image.save(self.map_tile_cache_file_name(map_tile_descriptor))?;
        }
        let map_tile = MapTile {
            descriptor: map_tile_descriptor.to_owned(),
            image,
        };
        Ok(Some(map_tile))
    }
}

/// represents a map assembled from map tiles
#[derive(Debug, Clone)]
pub struct Map {
    /// the grid rectangle of regions represented by this map
    grid_rectangle: GridRectangle,
    /// the actual map image
    image: image::DynamicImage,
}

/// represents errors that can occur while creating a map
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    /// an error in the map tile cache
    #[error("error in map tile cache while assembling map: {0}")]
    MapTileCacheError(#[from] MapTileCacheError),
    /// an error occurred when trying to calculate the zoom level that fits the
    /// map grid rectangle into the output image
    #[error("error when trying to calculate zoom level that fits the map grid rectangle into the output image: {0}")]
    ZoomFitError(#[from] ZoomFitError),
}

impl Map {
    /// creates a new `Map`
    ///
    /// # Errors
    ///
    /// returns an error if fetching the map tiles fails
    ///
    /// # Arguments
    ///
    /// * `map_tile_cache` - the map tile cache to use to fetch the map tiles
    /// * `x` - the width of the map in pixels
    /// * `y` - the height of the map in pixels
    /// * `grid_rectangle` - the grid rectangle of regions represented by this map
    pub async fn new(
        map_tile_cache: &MapTileCache,
        x: u32,
        y: u32,
        grid_rectangle: GridRectangle,
    ) -> Result<Self, MapError> {
        let zoom_level = ZoomLevel::max_zoom_level_to_fit_regions_into_output_image(
            grid_rectangle.size_x(),
            grid_rectangle.size_y(),
            x,
            y,
        )?;
        let actual_x =
            zoom_level.tile_size_in_pixels() * <u16 as Into<u32>>::into(grid_rectangle.size_x());
        let actual_y =
            zoom_level.tile_size_in_pixels() * <u16 as Into<u32>>::into(grid_rectangle.size_y());
        tracing::debug!("Determined max zoom level for map of size ({x}, {y}) for {grid_rectangle:?} to be {zoom_level:?}, actual map size will be ({actual_x}, {actual_y})");
        let x = actual_x;
        let y = actual_y;
        let mut image = image::DynamicImage::new_rgb8(x, y);
        for region_x in grid_rectangle.x_range() {
            for region_y in grid_rectangle.y_range() {
                let grid_coordinates = GridCoordinates::new(region_x, region_y);
                let map_tile_descriptor = MapTileDescriptor::new(zoom_level, grid_coordinates);
                tracing::debug!("Map tile for {grid_coordinates:?} is {map_tile_descriptor:?}");
                if let Some(map_tile) = map_tile_cache.get_map_tile(&map_tile_descriptor).await? {
                    let crop_x = (zoom_level.region_size_in_map_tile_in_pixels()
                        * (region_x - map_tile_descriptor.lower_left_corner().x()))
                    .into();
                    let crop_y: u32 = (zoom_level.region_size_in_map_tile_in_pixels()
                        * (region_y - map_tile_descriptor.lower_left_corner().y() + 1))
                        .into();
                    let crop_y = zoom_level.tile_size_in_pixels() - crop_y;
                    let crop_width = zoom_level.region_size_in_map_tile_in_pixels().into();
                    let crop_height = zoom_level.region_size_in_map_tile_in_pixels().into();
                    tracing::debug!(
                        "Cropping map tile to ({crop_x}, {crop_y})+{crop_width}x{crop_height}"
                    );
                    let crop = image::imageops::crop_imm(
                        map_tile.image(),
                        crop_x,
                        crop_y,
                        crop_width,
                        crop_height,
                    );
                    let region_x: i64 = region_x.into();
                    let region_y: i64 = region_y.into();
                    let offset_x: i64 =
                        region_x - <u16 as Into<i64>>::into(grid_rectangle.lower_left_corner().x());
                    let offset_y: i64 =
                        region_y - <u16 as Into<i64>>::into(grid_rectangle.lower_left_corner().y());
                    let region_size_in_map_tile_in_pixels: i64 =
                        zoom_level.region_size_in_map_tile_in_pixels().into();
                    let y: i64 = y.into();
                    let replace_x = offset_x * region_size_in_map_tile_in_pixels;
                    let replace_y = y - ((offset_y + 1) * region_size_in_map_tile_in_pixels);
                    tracing::debug!("Replacing target image ({replace_x}, {replace_y})+{crop_width}x{crop_height}");
                    image::imageops::replace(&mut image, &*crop, replace_x, replace_y);
                }
            }
        }
        Ok(Self {
            grid_rectangle,
            image,
        })
    }

    /// returns the grid rectangle this map represents
    #[must_use]
    pub fn grid_rectangle(&self) -> &GridRectangle {
        &self.grid_rectangle
    }

    /// returns the image for this map
    #[must_use]
    pub fn image(&self) -> &image::DynamicImage {
        &self.image
    }

    /// saves the map to the specified path
    ///
    /// # Errors
    ///
    /// returns an error when the image libraries returns an error
    /// when saving the image
    pub fn save(&self, path: &std::path::Path) -> Result<(), image::ImageError> {
        self.image.save(path)
    }
}

#[cfg(test)]
mod test {
    use sl_types::map::{GridCoordinates, ZoomLevel};
    use tracing_test::traced_test;

    use super::*;

    #[tokio::test]
    async fn test_fetch_map_tile_highest_detail() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
        map_tile_cache
            .get_map_tile(&MapTileDescriptor::new(
                ZoomLevel::try_new(1)?,
                GridCoordinates::new(1136, 1075),
            ))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_map_tile_highest_detail_twice() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
        map_tile_cache
            .get_map_tile(&MapTileDescriptor::new(
                ZoomLevel::try_new(1)?,
                GridCoordinates::new(1136, 1075),
            ))
            .await?;
        map_tile_cache
            .get_map_tile(&MapTileDescriptor::new(
                ZoomLevel::try_new(1)?,
                GridCoordinates::new(1136, 1075),
            ))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_map_tile_lowest_detail() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
        map_tile_cache
            .get_map_tile(&MapTileDescriptor::new(
                ZoomLevel::try_new(8)?,
                GridCoordinates::new(1136, 1075),
            ))
            .await?;
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    async fn test_fetch_map_zoom_level_1() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let ratelimiter =
            ratelimit::Ratelimiter::builder(1, std::time::Duration::from_secs(1)).build()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &map_tile_cache,
            512,
            512,
            GridRectangle::new(
                GridCoordinates::new(1135, 1070),
                GridCoordinates::new(1136, 1071),
            ),
        )
        .await?;
        map.save(std::path::Path::new("/tmp/test_map_zoom_level_1.jpg"))?;
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    async fn test_fetch_map_zoom_level_2() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let ratelimiter =
            ratelimit::Ratelimiter::builder(1, std::time::Duration::from_secs(1)).build()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &map_tile_cache,
            256,
            256,
            GridRectangle::new(
                GridCoordinates::new(1136, 1074),
                GridCoordinates::new(1137, 1075),
            ),
        )
        .await?;
        map.save(std::path::Path::new("/tmp/test_map_zoom_level_2.jpg"))?;
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    async fn test_fetch_map_zoom_level_3() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let ratelimiter =
            ratelimit::Ratelimiter::builder(1, std::time::Duration::from_secs(1)).build()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &map_tile_cache,
            128,
            128,
            GridRectangle::new(
                GridCoordinates::new(1136, 1074),
                GridCoordinates::new(1137, 1075),
            ),
        )
        .await?;
        map.save(std::path::Path::new("/tmp/test_map_zoom_level_3.jpg"))?;
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    async fn test_fetch_map_zoom_level_1_ratelimiter() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let ratelimiter =
            ratelimit::Ratelimiter::builder(1, std::time::Duration::from_millis(100)).build()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &map_tile_cache,
            2048,
            2048,
            GridRectangle::new(
                GridCoordinates::new(1131, 1068),
                GridCoordinates::new(1139, 1075),
            ),
        )
        .await?;
        map.save(std::path::Path::new(
            "/tmp/test_map_zoom_level_1_ratelimiter.jpg",
        ))?;
        Ok(())
    }
}
