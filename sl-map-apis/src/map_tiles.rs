//! Contains functionality related to fetching map tiles
use sl_types::map::{GridCoordinates, ZoomLevel};

/// represents a map tile fetched from the server
#[derive(Debug, Clone)]
pub struct MapTile {
    /// the zoom level of the map tile
    zoom_level: ZoomLevel,

    /// the lower left corner of the map tile
    lower_left_corner: GridCoordinates,

    /// the actual image data
    image: image::DynamicImage,
}

/// errors that can happen while fetching a map tile from the server
#[derive(Debug, thiserror::Error)]
pub enum MapTileFetchError {
    /// reqwest error when fetching the map tile from the server
    #[error("reqwest error when fetching the map tile from the server: {0}")]
    ReqwestError(#[from] reqwest::Error),
    /// error reading the raw map tile into an image
    #[error("error reading the raw map tile into an image: {0}")]
    ImageError(#[from] image::ImageError),
}

impl MapTile {
    /// fetches a map tile from the Second Life main map servers
    ///
    /// # Errors
    ///
    /// returns an error if the HTTP request fails
    pub async fn fetch(
        client: &reqwest::Client,
        grid_coordinates: &GridCoordinates,
        zoom_level: &ZoomLevel,
    ) -> Result<MapTile, MapTileFetchError> {
        let lower_left_corner = zoom_level.map_tile_corner(grid_coordinates);
        let url = format!(
            "https://secondlife-maps-cdn.akamaized.net/map-{}-{}-{}-objects.jpg",
            zoom_level,
            lower_left_corner.x(),
            lower_left_corner.y()
        );
        let response = client.get(&url).send().await?;
        dbg!(response.headers());
        let raw_response_body = response.bytes().await?;
        let image = image::ImageReader::new(std::io::Cursor::new(raw_response_body)).decode()?;
        Ok(MapTile {
            zoom_level: zoom_level.to_owned(),
            lower_left_corner,
            image,
        })
    }

    /// the zoom level of the map tile
    #[must_use]
    pub fn zoom_level(&self) -> &ZoomLevel {
        &self.zoom_level
    }

    /// the lower left corner of the map tile
    #[must_use]
    pub fn lower_left_corner(&self) -> &GridCoordinates {
        &self.lower_left_corner
    }

    /// the image data of the map tile
    #[must_use]
    pub fn image(&self) -> &image::DynamicImage {
        &self.image
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_fetch_map_tile_highest_detail() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        MapTile::fetch(
            &client,
            &GridCoordinates::new(1136, 1075),
            &ZoomLevel::try_new(1)?,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_map_tile_lowest_detail() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        MapTile::fetch(
            &client,
            &GridCoordinates::new(1136, 1075),
            &ZoomLevel::try_new(8)?,
        )
        .await?;
        Ok(())
    }
}
