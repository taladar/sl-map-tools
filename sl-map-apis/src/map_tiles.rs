//! Contains functionality related to fetching map tiles
use std::path::PathBuf;

use image::GenericImageView as _;
use sl_types::map::{
    GridCoordinateOffset, GridCoordinates, GridRectangle, GridRectangleLike, MapTileDescriptor,
    RegionCoordinates, RegionName, USBNotecard, ZoomFitError, ZoomLevel, ZoomLevelError,
};

use crate::region::RegionNameToGridCoordinatesCache;

/// represents a map like image, e.g. a map tile or a map that covers
/// some `GridRectangle` of regions
pub trait MapLike: GridRectangleLike + image::GenericImage + image::GenericImageView {
    /// the image of the map
    #[must_use]
    fn image(&self) -> &image::DynamicImage;

    /// the mutable image of the map
    #[must_use]
    fn image_mut(&mut self) -> &mut image::DynamicImage;

    /// the zoom level of the map
    #[must_use]
    fn zoom_level(&self) -> ZoomLevel;

    /// pixels per meter
    #[must_use]
    fn pixels_per_meter(&self) -> f32 {
        self.zoom_level().pixels_per_meter()
    }

    /// pixels per region
    #[must_use]
    fn pixels_per_region(&self) -> f32 {
        self.pixels_per_meter() * 256f32
    }

    /// the pixel coordinates in the map that represent the given `GridCoordinates`
    /// and `RegionCoordinates`
    #[must_use]
    fn pixel_coordinates_for_coordinates(
        &self,
        grid_coordinates: &GridCoordinates,
        region_coordinates: &RegionCoordinates,
    ) -> Option<(u32, u32)> {
        if !self.contains(grid_coordinates) {
            return None;
        }
        let grid_offset = *grid_coordinates - self.lower_left_corner();
        let x = (self.pixels_per_region() * grid_offset.x() as f32
            + self.pixels_per_meter() * region_coordinates.x()) as u32;
        let y = (self.pixels_per_region() * grid_offset.y() as f32
            + self.pixels_per_meter() * region_coordinates.y()) as u32;
        let y = self.height() - y;
        Some((x, y))
    }

    /// the `GridCoordinates` and `RegionCoordinates` at the given pixel coordinates
    #[must_use]
    fn coordinates_for_pixel_coordinates(
        &self,
        x: u32,
        y: u32,
    ) -> Option<(GridCoordinates, RegionCoordinates)> {
        if !(x <= self.dimensions().0 && y <= self.dimensions().1) {
            return None;
        }
        let y = self.height() - y;
        let grid_result = self.lower_left_corner()
            + GridCoordinateOffset::new(
                (x as f32 / self.pixels_per_region()) as i32,
                (y as f32 / self.pixels_per_region()) as i32,
            );
        let region_result = RegionCoordinates::new(
            (x % self.pixels_per_region() as u32) as f32 / self.pixels_per_meter(),
            (y % self.pixels_per_region() as u32) as f32 / self.pixels_per_meter(),
            0f32,
        );
        Some((grid_result, region_result))
    }

    /// a crop of the map like image by coordinates and size
    #[must_use]
    fn crop_imm_grid_rectangle(
        &self,
        grid_rectangle: &GridRectangle,
    ) -> Option<image::SubImage<&Self>>
    where
        Self: Sized,
    {
        let lower_left_corner_pixels = self.pixel_coordinates_for_coordinates(
            &grid_rectangle.lower_left_corner(),
            &RegionCoordinates::new(0f32, 0f32, 0f32),
        )?;
        let upper_right_corner_pixels = self.pixel_coordinates_for_coordinates(
            &grid_rectangle.upper_right_corner(),
            &RegionCoordinates::new(256f32, 256f32, 0f32),
        )?;
        let x = std::cmp::min(lower_left_corner_pixels.0, upper_right_corner_pixels.0);
        let y = std::cmp::min(lower_left_corner_pixels.1, upper_right_corner_pixels.1);
        let width = lower_left_corner_pixels
            .0
            .abs_diff(upper_right_corner_pixels.0);
        let height = lower_left_corner_pixels
            .1
            .abs_diff(upper_right_corner_pixels.1);
        Some(image::imageops::crop_imm(self, x, y, width, height))
    }

    /// draw a waypoint at the given coordinates
    fn draw_waypoint(&mut self, x: u32, y: u32, color: image::Rgba<u8>) {
        imageproc::drawing::draw_filled_rect_mut(
            self.image_mut(),
            imageproc::rect::Rect::at((x - 10) as i32, (y - 10) as i32).of_size(20, 20),
            color,
        );
    }

    /// draw a line from the given coordinates to the given coordinates
    fn draw_line(
        &mut self,
        from_x: u32,
        from_y: u32,
        to_x: u32,
        to_y: u32,
        color: image::Rgba<u8>,
    ) {
        let from_x = from_x as f32;
        let from_y = from_y as f32;
        let to_x = to_x as f32;
        let to_y = to_y as f32;
        let diff = (to_x - from_x, to_y - from_y);
        let perpendicular = (-diff.1, diff.0);
        let magnitude = (diff.0.powi(2) + diff.1.powi(2)).sqrt();
        let perpendicular_normalized = (perpendicular.0 / magnitude, perpendicular.1 / magnitude);
        let points = vec![
            imageproc::point::Point::new(
                (from_x + perpendicular_normalized.0 * 5.0) as i32,
                (from_y + perpendicular_normalized.1 * 5.0) as i32,
            ),
            imageproc::point::Point::new(
                (to_x + perpendicular_normalized.0 * 5.0) as i32,
                (to_y + perpendicular_normalized.1 * 5.0) as i32,
            ),
            imageproc::point::Point::new(
                (to_x - perpendicular_normalized.0 * 5.0) as i32,
                (to_y - perpendicular_normalized.1 * 5.0) as i32,
            ),
            imageproc::point::Point::new(
                (from_x - perpendicular_normalized.0 * 5.0) as i32,
                (from_y - perpendicular_normalized.1 * 5.0) as i32,
            ),
        ];
        imageproc::drawing::draw_antialiased_polygon_mut(
            self.image_mut(),
            &points,
            color,
            imageproc::pixelops::interpolate,
        );
    }
}

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
}

impl GridRectangleLike for MapTile {
    fn grid_rectangle(&self) -> GridRectangle {
        self.descriptor.grid_rectangle()
    }
}

impl image::GenericImageView for MapTile {
    type Pixel = <image::DynamicImage as image::GenericImageView>::Pixel;

    fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.image.get_pixel(x, y)
    }
}

impl image::GenericImage for MapTile {
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        #[allow(deprecated)]
        self.image.get_pixel_mut(x, y)
    }

    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image.put_pixel(x, y, pixel)
    }

    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        #[allow(deprecated)]
        self.image.blend_pixel(x, y, pixel)
    }
}

impl MapLike for MapTile {
    fn zoom_level(&self) -> ZoomLevel {
        self.descriptor.zoom_level().to_owned()
    }

    fn image(&self) -> &image::DynamicImage {
        &self.image
    }

    fn image_mut(&mut self) -> &mut image::DynamicImage {
        &mut self.image
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
    /// error creating a zoom level
    #[error("error creating a zoom level: {0}")]
    ZoomLevelError(#[from] ZoomLevelError),
    /// error when trying to load cache policy that we previously checked
    /// existed on disk
    #[error("error when trying to load cache policy that we previously checked existed on disk")]
    CachePolicyError,
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
    /// the in-memory cache
    #[debug(skip)]
    cache: lru::LruCache<MapTileDescriptor, (Option<MapTile>, http_cache_semantics::CachePolicy)>,
}

/// status of a cache entry on disk
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapTileCacheEntryStatus {
    /// no files at all related to a map tile in the cache
    Missing,
    /// an incomplete set of files related to a map tile in the cache
    Invalid,
    /// a usable set of files related to a map tile in the cache (cache policy + either a map tile or an absence marker)
    Valid,
}

/// a wrapper around response to force status from 403 to 404 for absent map
/// tiles so `http_cache_semantics::CachePolicy` becomes usable on those responses
#[derive(Debug)]
pub struct MapTileNegativeResponse(reqwest::Response);

impl http_cache_semantics::ResponseLike for MapTileNegativeResponse {
    fn status(&self) -> http::status::StatusCode {
        match self.0.status() {
            http::status::StatusCode::FORBIDDEN => http::status::StatusCode::NOT_FOUND,
            status => status,
        }
    }

    fn headers(&self) -> &http::header::HeaderMap {
        self.0.headers()
    }
}

impl MapTileCache {
    /// creates a new `MapTileCache`
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn new(cache_directory: PathBuf, ratelimiter: Option<ratelimit::Ratelimiter>) -> Self {
        // unwrap is okay here because we know that the literal 16 is non-zero
        // same reason for missing_panics_doc above
        #[allow(clippy::unwrap_used)]
        let cache = lru::LruCache::new(std::num::NonZeroUsize::new(16).unwrap());
        MapTileCache {
            client: reqwest::Client::new(),
            ratelimiter,
            cache_directory,
            cache,
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

    /// the file name marking a negative response in the cache directory
    #[must_use]
    fn map_tile_cache_negative_response_file_name(
        &self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> PathBuf {
        self.cache_directory.join(format!(
            "{}.does-not-exist",
            self.map_tile_file_name(map_tile_descriptor)
        ))
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

    /// check if a cache entry is missing, invalid or valid (either cache policy + map tile or cache policy + negative response)
    async fn cache_entry_status(
        &self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<MapTileCacheEntryStatus, MapTileCacheError> {
        match (
            self.cache_policy_file_name(map_tile_descriptor).exists(),
            self.map_tile_cache_file_name(map_tile_descriptor).exists(),
            self.map_tile_cache_negative_response_file_name(map_tile_descriptor)
                .exists(),
        ) {
            (false, false, false) => Ok(MapTileCacheEntryStatus::Missing),
            (true, true, false) => Ok(MapTileCacheEntryStatus::Valid),
            (true, false, true) => Ok(MapTileCacheEntryStatus::Valid),
            (cp, tile, neg) => {
                tracing::warn!(
                    "cache entry status is invalid: cache policy file: {}, map tile file: {}, negative response file: {}",
                    cp, tile, neg
                );
                Ok(MapTileCacheEntryStatus::Invalid)
            }
        }
    }

    /// loads the cached `MapTile` and cache policy from the cache directory
    /// or from the in-memory LRU cache
    ///
    /// # Errors
    ///
    /// returns an error if file operations fail
    async fn fetch_cached_map_tile(
        &mut self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<Option<(Option<MapTile>, http_cache_semantics::CachePolicy)>, MapTileCacheError>
    {
        if let Some(cache_entry) = self.cache.get(map_tile_descriptor) {
            return Ok(Some(cache_entry.to_owned()));
        }
        let cache_file = self.map_tile_cache_file_name(map_tile_descriptor);
        let cache_entry_status = self.cache_entry_status(map_tile_descriptor).await?;
        if cache_entry_status == MapTileCacheEntryStatus::Invalid {
            self.remove_cached_tile(map_tile_descriptor).await?;
            return Ok(None);
        }
        if cache_entry_status == MapTileCacheEntryStatus::Missing {
            return Ok(None);
        }
        let Some(cache_policy) = self.load_cache_policy(map_tile_descriptor).await? else {
            return Err(MapTileCacheError::CachePolicyError);
        };
        if cache_file.exists() {
            let cached_map_tile = image::ImageReader::open(cache_file)
                .map_err(MapTileCacheError::CacheDirectoryFileError)?
                .decode()?;
            Ok(Some((
                Some(MapTile {
                    descriptor: map_tile_descriptor.to_owned(),
                    image: cached_map_tile,
                }),
                cache_policy,
            )))
        } else {
            // since we know the cache entry status is valid and no map tile exists we must be dealing with a cached absence
            Ok(Some((None, cache_policy)))
        }
    }

    /// clears the data about a specific map tile from the cache
    async fn remove_cached_tile(
        &mut self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<(), MapTileCacheError> {
        tracing::debug!("Removing {map_tile_descriptor:?} from map tile cache");
        self.cache.pop(map_tile_descriptor);
        let cache_file = self.map_tile_cache_file_name(map_tile_descriptor);
        let cache_file_negative_response =
            self.map_tile_cache_negative_response_file_name(map_tile_descriptor);
        let cache_policy_file = self.cache_policy_file_name(map_tile_descriptor);
        if cache_file.exists() {
            std::fs::remove_file(cache_file).map_err(MapTileCacheError::CacheDirectoryFileError)?;
        }
        if cache_file_negative_response.exists() {
            std::fs::remove_file(cache_file_negative_response)
                .map_err(MapTileCacheError::CacheDirectoryFileError)?;
        }
        if cache_policy_file.exists() {
            std::fs::remove_file(cache_policy_file)
                .map_err(MapTileCacheError::CacheDirectoryFileError)?;
        }
        Ok(())
    }

    /// loads the `http_cache_semantics::CachePolicy` for a cached map tile
    /// or absence from disk cache
    ///
    /// # Errors
    ///
    /// returns an error if file operations or JSON deserialization fail
    async fn load_cache_policy(
        &self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<Option<http_cache_semantics::CachePolicy>, MapTileCacheError> {
        let cache_policy_file = self.cache_policy_file_name(map_tile_descriptor);
        if !cache_policy_file.exists() {
            return Ok(None);
        }
        let cache_policy = std::fs::read_to_string(cache_policy_file)
            .map_err(MapTileCacheError::CacheDirectoryFileError)?;
        Ok(serde_json::from_str(&cache_policy)?)
    }

    /// stores the cache policy in the disk cache
    ///
    /// # Errors
    ///
    /// returns an error if there was an error in the file operation or when
    /// serializing the cache policy
    async fn store_cache_policy(
        &self,
        map_tile_descriptor: &MapTileDescriptor,
        cache_policy: http_cache_semantics::CachePolicy,
    ) -> Result<(), MapTileCacheError> {
        if !self.cache_directory.exists() {
            std::fs::create_dir_all(&self.cache_directory)
                .map_err(MapTileCacheError::CacheDirectoryFileError)?;
        }
        let cache_policy = serde_json::to_string(&cache_policy)?;
        std::fs::write(
            self.cache_policy_file_name(map_tile_descriptor),
            cache_policy,
        )
        .map_err(MapTileCacheError::CacheDirectoryFileError)?;
        Ok(())
    }

    /// marks a tile as missing in the cache if the cache policy indicates
    /// it is storable
    ///
    /// # Errors
    ///
    /// returns an error if there was an error in the file operations
    /// or serialization of the cache policy
    async fn cache_missing_tile(
        &mut self,
        map_tile_descriptor: &MapTileDescriptor,
        cache_policy: http_cache_semantics::CachePolicy,
    ) -> Result<(), MapTileCacheError> {
        if cache_policy.is_storable() {
            tracing::debug!("Caching absence of map tile {map_tile_descriptor:?}");
            self.store_cache_policy(map_tile_descriptor, cache_policy.to_owned())
                .await?;
            let cache_file_negative_response =
                self.map_tile_cache_negative_response_file_name(map_tile_descriptor);
            std::fs::File::create(cache_file_negative_response)
                .map_err(MapTileCacheError::CacheDirectoryFileError)?;
            self.cache
                .put(map_tile_descriptor.clone(), (None, cache_policy));
        } else {
            tracing::warn!("Absence of map tile {map_tile_descriptor:?} not storable according to cache policy");
        }
        Ok(())
    }

    /// stores a tile in the cache if the cache policy indicates that
    /// it is storable
    ///
    /// # Errors
    ///
    /// returns an error if there was an error in the file operations
    /// or serialization of the cache policy
    async fn cache_tile(
        &mut self,
        map_tile_descriptor: &MapTileDescriptor,
        map_tile: &MapTile,
        cache_policy: http_cache_semantics::CachePolicy,
    ) -> Result<(), MapTileCacheError> {
        if cache_policy.is_storable() {
            tracing::debug!("Caching map tile {map_tile_descriptor:?}");
            self.store_cache_policy(map_tile_descriptor, cache_policy.to_owned())
                .await?;
            map_tile
                .image
                .save(self.map_tile_cache_file_name(map_tile_descriptor))?;
            self.cache.put(
                map_tile_descriptor.clone(),
                (Some(map_tile.to_owned()), cache_policy),
            );
        } else {
            tracing::warn!(
                "Map tile {map_tile_descriptor:?} not storable according to cache policy"
            );
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
        &mut self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<Option<MapTile>, MapTileCacheError> {
        tracing::debug!("Map tile {map_tile_descriptor:?} requested");
        let url = self.map_tile_url(map_tile_descriptor);
        let request = self.client.get(&url).build()?;
        let now = std::time::SystemTime::now();
        if let Some((cached_map_tile, cache_policy)) =
            self.fetch_cached_map_tile(map_tile_descriptor).await?
        {
            if cached_map_tile.is_some() {
                tracing::debug!("Found matching map tile in cache, checking freshness");
            } else {
                tracing::debug!("Found matching map tile absence in cache, checking freshness");
            }
            if let http_cache_semantics::BeforeRequest::Fresh(_) =
                cache_policy.before_request(&request, now)
            {
                if cached_map_tile.is_some() {
                    tracing::debug!("Using cached map tile");
                } else {
                    tracing::debug!("Using cached map tile absence");
                }
                return Ok(cached_map_tile);
            }
            tracing::debug!("Map tile cache not fresh, removing from cache");
            self.remove_cached_tile(map_tile_descriptor).await?;
        }
        tracing::debug!("Waiting for ratelimiter to fetch map tile from server");
        if let Some(ratelimiter) = &self.ratelimiter {
            while let Err(duration) = ratelimiter.try_wait() {
                tokio::time::sleep(duration).await;
            }
        }
        tracing::debug!("Fetching map tile from server at {}", url);
        let response = self
            .client
            .execute(
                request
                    .try_clone()
                    .ok_or(MapTileCacheError::FailedToCloneRequest)?,
            )
            .await?;
        tracing::debug!(
            "Server response received: status {}, headers\n{:#?}",
            response.status(),
            response.headers()
        );
        if !response.status().is_success() {
            if response.status() == reqwest::StatusCode::FORBIDDEN {
                // FORBIDDEN (403) is returned when the file does not exist
                // which likely means there is no region/map tile
                tracing::debug!("Received 403 FORBIDDEN response, interpreting as no map tile for these grid coordinates");
                let cache_policy = http_cache_semantics::CachePolicy::new(
                    &request,
                    &MapTileNegativeResponse(response),
                );
                self.cache_missing_tile(map_tile_descriptor, cache_policy)
                    .await?;
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
        tracing::debug!("Parsing received map tile to image");
        let image = image::ImageReader::new(std::io::Cursor::new(raw_response_body))
            .with_guessed_format()
            .map_err(MapTileCacheError::ImageFormatGuessError)?
            .decode()?;
        let map_tile = MapTile {
            descriptor: map_tile_descriptor.to_owned(),
            image,
        };
        self.cache_tile(map_tile_descriptor, &map_tile, cache_policy)
            .await?;
        tracing::debug!("Returning freshly fetched map tile");
        Ok(Some(map_tile))
    }

    /// figures out if a map tile exist by checking the local in-memory and
    /// disk caches or fetching the map tile from the server
    ///
    /// # Errors
    ///
    /// returns an error if fetching the map tile from cache or remotely fails
    pub async fn does_map_tile_exist(
        &mut self,
        map_tile_descriptor: &MapTileDescriptor,
    ) -> Result<bool, MapTileCacheError> {
        let url = self.map_tile_url(map_tile_descriptor);
        if let Some((map_tile, cache_policy)) = self.cache.get(map_tile_descriptor) {
            let request = self.client.get(&url).build()?;
            let now = std::time::SystemTime::now();
            if let http_cache_semantics::BeforeRequest::Fresh(_) =
                cache_policy.before_request(&request, now)
            {
                return Ok(map_tile.is_some());
            }
        }
        if self.cache_entry_status(&map_tile_descriptor).await? == MapTileCacheEntryStatus::Valid {
            if let Some(cache_policy) = self.load_cache_policy(&map_tile_descriptor).await? {
                let request = self.client.get(&url).build()?;
                let now = std::time::SystemTime::now();
                if let http_cache_semantics::BeforeRequest::Fresh(_) =
                    cache_policy.before_request(&request, now)
                {
                    if self
                        .map_tile_cache_negative_response_file_name(map_tile_descriptor)
                        .exists()
                    {
                        return Ok(false);
                    }
                    return Ok(true);
                }
            }
        }
        Ok(self.get_map_tile(&map_tile_descriptor).await?.is_some())
    }

    /// figures out if a region exists based on the existence of map tiles for it, starting with the lowest zoom level
    /// and potentially going up to the highest one if all the other zoom levels have a tile for that region
    ///
    /// # Errors
    ///
    /// returns an error if fetching map tiles from cache or remotely fails
    pub async fn does_region_exist(
        &mut self,
        grid_coordinates: &GridCoordinates,
    ) -> Result<bool, MapTileCacheError> {
        for zoom_level in (1..=8).rev() {
            tracing::debug!("Checking if zoom level {zoom_level} map tile exists for region {grid_coordinates:?}");
            let map_tile_descriptor = MapTileDescriptor::new(
                ZoomLevel::try_new(zoom_level)?,
                grid_coordinates.to_owned(),
            );
            if !self.does_map_tile_exist(&map_tile_descriptor).await? {
                tracing::debug!("No map tile found, region {grid_coordinates:?} does not exist");
                return Ok(false);
            }
            let cache_entry_status = self.cache_entry_status(&map_tile_descriptor).await?;
            if cache_entry_status == MapTileCacheEntryStatus::Valid {}
        }
        tracing::debug!(
            "Map tiles exist for {grid_coordinates:?} on all zoom levels, region exists"
        );
        Ok(true)
    }
}

/// represents a map assembled from map tiles
#[derive(Debug, Clone)]
pub struct Map {
    /// the zoom level of this map
    zoom_level: ZoomLevel,
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
    /// failed to crop a map tile to the required size
    #[error("error when cropping a map tile to the required size")]
    MapTileCropError,
    /// failed to calculate pixel coordinates where we want to place a map tile crop
    #[error("error when calculating pixel coordinates where we want to place a map tile crop")]
    MapCoordinateError,
    /// no overlap between map tile we fetched and output map (should not happen)
    #[error("no overlap between map tile we fetched and output map (should not happen)")]
    NoOverlapError,
    /// no grid coordinates were returned for one of the region names in the
    /// USB Notecard
    #[error("No grid coordinates were returned for one of the regions in the USB notecard: {0}")]
    NoGridCoordinatesForRegion(RegionName),
    /// error in region name to grid coordinate cache
    #[error("error in region name to grid coordinate cache: {0}")]
    RegionNameToGridCoordinateCacheError(#[from] crate::region::CacheError),
}

impl Map {
    /// creates a new `Map`
    ///
    /// if we choose not to fill the missing map tiles they appear as black
    ///
    /// if we choose not to fill the missing regions they appear in a color
    /// similar to water but filling them in has some performance impact since
    /// we need to check if the region exists by fetching higher resolutio
    /// map tiles for it.
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
        map_tile_cache: &mut MapTileCache,
        x: u32,
        y: u32,
        grid_rectangle: GridRectangle,
        fill_missing_map_tiles: Option<image::Rgba<u8>>,
        fill_missing_regions: Option<image::Rgba<u8>>,
    ) -> Result<Self, MapError> {
        let zoom_level = ZoomLevel::max_zoom_level_to_fit_regions_into_output_image(
            grid_rectangle.size_x(),
            grid_rectangle.size_y(),
            x,
            y,
        )?;
        let actual_x = <u16 as Into<u32>>::into(zoom_level.pixels_per_region())
            * <u16 as Into<u32>>::into(grid_rectangle.size_x());
        let actual_y = <u16 as Into<u32>>::into(zoom_level.pixels_per_region())
            * <u16 as Into<u32>>::into(grid_rectangle.size_y());
        tracing::debug!("Determined max zoom level for map of size ({x}, {y}) for {grid_rectangle:?} to be {zoom_level:?}, actual map size will be ({actual_x}, {actual_y})");
        let x = actual_x;
        let y = actual_y;
        let image = image::DynamicImage::new_rgb8(x, y);
        let mut result = Self {
            zoom_level,
            grid_rectangle,
            image,
        };
        for region_x in result.x_range() {
            for region_y in result.y_range() {
                let grid_coordinates = GridCoordinates::new(region_x, region_y);
                let map_tile_descriptor = MapTileDescriptor::new(zoom_level, grid_coordinates);
                let Some(overlap) = result.intersect(&map_tile_descriptor) else {
                    return Err(MapError::NoOverlapError);
                };
                if overlap.lower_left_corner().x() != region_x
                    || overlap.lower_left_corner().y() != region_y
                {
                    // we should have already processed this map tile when
                    // we encountered the lower left corner of the overlap
                    continue;
                }
                tracing::debug!("Map tile for {grid_coordinates:?} is {map_tile_descriptor:?}");
                if let Some(map_tile) = map_tile_cache.get_map_tile(&map_tile_descriptor).await? {
                    let crop = map_tile
                        .crop_imm_grid_rectangle(&overlap)
                        .ok_or(MapError::MapTileCropError)?;
                    tracing::debug!(
                        "Cropped map tile to ({}, {})+{}x{}",
                        crop.offsets().0,
                        crop.offsets().1,
                        (*crop).dimensions().0,
                        (*crop).dimensions().1
                    );
                    // we need to use y = 256 here since the crop is inserted by pixel coordinates which means
                    // we need the upper left corner, not the lower left one of the region as an origin
                    let (replace_x, replace_y) = result
                        .pixel_coordinates_for_coordinates(
                            &overlap.upper_left_corner(),
                            &RegionCoordinates::new(0f32, 256f32, 0f32),
                        )
                        .ok_or(MapError::MapCoordinateError)?;
                    tracing::debug!(
                        "Placing map tile crop at ({replace_x}, {replace_y}) in the output image"
                    );
                    image::imageops::replace(
                        &mut result,
                        &*crop,
                        replace_x.into(),
                        replace_y.into(),
                    );
                    if let Some(fill_color) = fill_missing_regions {
                        for overlap_region_x in overlap.x_range() {
                            for overlap_region_y in overlap.y_range() {
                                let grid_coordinates =
                                    GridCoordinates::new(overlap_region_x, overlap_region_y);
                                if !map_tile_cache.does_region_exist(&grid_coordinates).await? {
                                    let pixel_min = result.pixel_coordinates_for_coordinates(
                                        &grid_coordinates,
                                        &RegionCoordinates::new(0f32, 256f32, 0f32),
                                    );
                                    let pixel_max = result.pixel_coordinates_for_coordinates(
                                        &grid_coordinates,
                                        &RegionCoordinates::new(256f32, 0f32, 0f32),
                                    );
                                    if let (Some((min_x, min_y)), Some((max_x, max_y))) =
                                        (pixel_min, pixel_max)
                                    {
                                        for x in min_x..max_x {
                                            for y in min_y..max_y {
                                                <Map as image::GenericImage>::put_pixel(
                                                    &mut result,
                                                    x,
                                                    y,
                                                    fill_color,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    if let Some(fill_color) = fill_missing_map_tiles {
                        let (replace_x, replace_y) = result
                            .pixel_coordinates_for_coordinates(
                                &overlap.upper_left_corner(),
                                &RegionCoordinates::new(0f32, 256f32, 0f32),
                            )
                            .ok_or(MapError::MapCoordinateError)?;
                        let pixel_size_x =
                            overlap.size_x() as u32 * zoom_level.pixels_per_region() as u32;
                        let pixel_size_y =
                            overlap.size_y() as u32 * zoom_level.pixels_per_region() as u32;
                        for x in replace_x..replace_x + pixel_size_x {
                            for y in replace_y..replace_y + pixel_size_y {
                                <Map as image::GenericImage>::put_pixel(
                                    &mut result,
                                    x,
                                    y,
                                    fill_color,
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    /// draws a route from a `USBNotecard` onto the map
    ///
    /// # Errors
    ///
    /// fails if the region name to grid coordinate conversion fails
    /// or the conversion of those into pixel coordinates
    pub async fn draw_route(
        &mut self,
        region_name_to_grid_coordinates_cache: &mut RegionNameToGridCoordinatesCache,
        usb_notecard: &USBNotecard,
        color: image::Rgba<u8>,
    ) -> Result<(), MapError> {
        tracing::debug!("Drawing route:\n{:#?}", usb_notecard);
        let mut previous_waypoint: Option<(u32, u32)> = None;
        for waypoint in usb_notecard.waypoints() {
            let Some(grid_coordinates) = region_name_to_grid_coordinates_cache
                .get_grid_coordinates(waypoint.location().region_name())
                .await?
            else {
                return Err(MapError::NoGridCoordinatesForRegion(
                    waypoint.location().region_name().to_owned(),
                ));
            };
            let (x, y) = self
                .pixel_coordinates_for_coordinates(
                    &grid_coordinates,
                    &waypoint.region_coordinates(),
                )
                .ok_or(MapError::MapCoordinateError)?;
            tracing::debug!(
                "Drawing waypoint at ({x}, {y}) for location {:?}",
                waypoint.location()
            );
            self.draw_waypoint(x, y, color);
            if let Some((previous_x, previous_y)) = previous_waypoint {
                tracing::debug!("Drawing line from ({previous_x}, {previous_y}) to ({x}, {y})");
                self.draw_line(previous_x, previous_y, x, y, color);
            }
            previous_waypoint = Some((x, y));
        }
        Ok(())
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

impl GridRectangleLike for Map {
    fn grid_rectangle(&self) -> GridRectangle {
        self.grid_rectangle.to_owned()
    }
}

impl image::GenericImageView for Map {
    type Pixel = <image::DynamicImage as image::GenericImageView>::Pixel;

    fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.image.get_pixel(x, y)
    }
}

impl image::GenericImage for Map {
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        #[allow(deprecated)]
        self.image.get_pixel_mut(x, y)
    }

    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image.put_pixel(x, y, pixel)
    }

    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        #[allow(deprecated)]
        self.image.blend_pixel(x, y, pixel)
    }
}

impl MapLike for Map {
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

#[cfg(test)]
mod test {
    use image::GenericImageView;
    use sl_types::map::{GridCoordinates, ZoomLevel};
    use tracing_test::traced_test;

    use super::*;

    #[tokio::test]
    async fn test_fetch_map_tile_highest_detail() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let mut map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
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
        let mut map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
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
        let mut map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
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
        let mut map_tile_cache =
            MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &mut map_tile_cache,
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
        let mut map_tile_cache =
            MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &mut map_tile_cache,
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
        let mut map_tile_cache =
            MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &mut map_tile_cache,
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
        let mut map_tile_cache =
            MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &mut map_tile_cache,
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

    #[traced_test]
    #[tokio::test]
    #[allow(clippy::panic)]
    async fn test_map_tile_pixel_coordinates_for_coordinates_single_region(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let mut map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
        let Some(map_tile) = map_tile_cache
            .get_map_tile(&MapTileDescriptor::new(
                ZoomLevel::try_new(1)?,
                GridCoordinates::new(1136, 1075),
            ))
            .await?
        else {
            panic!("Expected there to be a region at this location");
        };
        for in_region_x in 0..=256 {
            for in_region_y in 0..=256 {
                let grid_coordinates = GridCoordinates::new(1136, 1075);
                let region_coordinates =
                    RegionCoordinates::new(in_region_x as f32, in_region_y as f32, 0f32);
                tracing::debug!("Now checking {grid_coordinates:?}, {region_coordinates:?}");
                assert_eq!(
                    map_tile
                        .pixel_coordinates_for_coordinates(&grid_coordinates, &region_coordinates,),
                    Some((in_region_x, 256 - in_region_y)),
                );
            }
        }
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    async fn test_map_pixel_coordinates_for_coordinates_four_regions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let ratelimiter =
            ratelimit::Ratelimiter::builder(1, std::time::Duration::from_secs(1)).build()?;
        let mut map_tile_cache =
            MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &mut map_tile_cache,
            512,
            512,
            GridRectangle::new(
                GridCoordinates::new(1136, 1074),
                GridCoordinates::new(1137, 1075),
            ),
        )
        .await?;
        for region_offset_x in 0..=1 {
            for region_offset_y in 0..=1 {
                for in_region_x in 0..=256 {
                    for in_region_y in 0..=256 {
                        let grid_coordinates =
                            GridCoordinates::new(1136 + region_offset_x, 1074 + region_offset_y);
                        let region_coordinates =
                            RegionCoordinates::new(in_region_x as f32, in_region_y as f32, 0f32);
                        tracing::debug!(
                            "Now checking {grid_coordinates:?}, {region_coordinates:?}"
                        );
                        assert_eq!(
                            map.pixel_coordinates_for_coordinates(
                                &grid_coordinates,
                                &region_coordinates,
                            ),
                            Some((
                                (region_offset_x * 256 + in_region_x) as u32,
                                (512 - (region_offset_y * 256 + in_region_y)) as u32
                            )),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    #[allow(clippy::panic)]
    async fn test_map_tile_coordinates_for_pixel_coordinates_single_region(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let mut map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf(), None);
        let Some(map_tile) = map_tile_cache
            .get_map_tile(&MapTileDescriptor::new(
                ZoomLevel::try_new(1)?,
                GridCoordinates::new(1136, 1075),
            ))
            .await?
        else {
            panic!("Expected there to be a region at this location");
        };
        tracing::debug!("Dimensions of map tile are {:?}", map_tile.dimensions());
        for in_region_x in 0..=256 {
            for in_region_y in 0..=256 {
                let pixel_x = in_region_x;
                let pixel_y = 256 - in_region_y;
                tracing::debug!("Now checking ({pixel_x}, {pixel_y})");
                assert_eq!(
                    map_tile.coordinates_for_pixel_coordinates(pixel_x, pixel_y,),
                    Some((
                        GridCoordinates::new(
                            1136 + if in_region_x == 256 { 1 } else { 0 },
                            1075 + if in_region_y == 256 { 1 } else { 0 }
                        ),
                        RegionCoordinates::new(
                            (in_region_x % 256) as f32,
                            (in_region_y % 256) as f32,
                            0f32
                        ),
                    ))
                );
            }
        }
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    async fn test_map_coordinates_for_pixel_coordinates_four_regions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let ratelimiter =
            ratelimit::Ratelimiter::builder(1, std::time::Duration::from_secs(1)).build()?;
        let mut map_tile_cache =
            MapTileCache::new(temp_dir.path().to_path_buf(), Some(ratelimiter));
        let map = Map::new(
            &mut map_tile_cache,
            512,
            512,
            GridRectangle::new(
                GridCoordinates::new(1136, 1074),
                GridCoordinates::new(1137, 1075),
            ),
        )
        .await?;
        tracing::debug!("Dimensions of map are {:?}", map.dimensions());
        for region_offset_x in 0..=1 {
            for region_offset_y in 0..=1 {
                for in_region_x in 0..=256 {
                    for in_region_y in 0..=256 {
                        let pixel_x = (region_offset_x * 256 + in_region_x) as u32;
                        let pixel_y = (512 - (region_offset_y * 256 + in_region_y)) as u32;
                        tracing::debug!("Now checking ({pixel_x}, {pixel_y})");
                        assert_eq!(
                            map.coordinates_for_pixel_coordinates(pixel_x, pixel_y,),
                            Some((
                                GridCoordinates::new(
                                    1136 + region_offset_x + if in_region_x == 256 { 1 } else { 0 },
                                    1074 + region_offset_y + if in_region_y == 256 { 1 } else { 0 }
                                ),
                                RegionCoordinates::new(
                                    (in_region_x % 256) as f32,
                                    (in_region_y % 256) as f32,
                                    0f32
                                ),
                            )),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
