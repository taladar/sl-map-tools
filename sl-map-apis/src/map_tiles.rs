//! Contains functionality related to fetching map tiles
use std::path::PathBuf;

use sl_types::map::MapTileDescriptor;

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
#[derive(Debug, Clone)]
pub struct MapTileCache {
    /// the client used to make HTTP requests for map tiles not in the local cache
    client: reqwest::Client,
    /// the cache directory
    cache_directory: PathBuf,
}

impl MapTileCache {
    /// creates a new `MapTileCache`
    #[must_use]
    pub fn new(cache_directory: PathBuf) -> Self {
        MapTileCache {
            client: reqwest::Client::new(),
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
    ) -> Result<MapTile, MapTileCacheError> {
        let url = self.map_tile_url(map_tile_descriptor);
        let request = self.client.get(&url).build()?;
        let now = std::time::SystemTime::now();
        if let Some((cached_map_tile, cache_policy)) =
            self.fetch_cached_map_tile(map_tile_descriptor).await?
        {
            if let http_cache_semantics::BeforeRequest::Fresh(_) =
                cache_policy.before_request(&request, now)
            {
                return Ok(cached_map_tile);
            }
            self.remove_cached_tile(map_tile_descriptor).await?;
        }
        let response = self
            .client
            .execute(
                request
                    .try_clone()
                    .ok_or(MapTileCacheError::FailedToCloneRequest)?,
            )
            .await?;
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
        Ok(map_tile)
    }
}

#[cfg(test)]
mod test {
    use sl_types::map::{GridCoordinates, ZoomLevel};

    use super::*;

    #[tokio::test]
    async fn test_fetch_map_tile_highest_detail() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf());
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
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf());
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
        let map_tile_cache = MapTileCache::new(temp_dir.path().to_path_buf());
        map_tile_cache
            .get_map_tile(&MapTileDescriptor::new(
                ZoomLevel::try_new(8)?,
                GridCoordinates::new(1136, 1075),
            ))
            .await?;
        Ok(())
    }
}
