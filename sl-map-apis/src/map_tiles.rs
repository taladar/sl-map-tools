//! Contains functionality related to fetching map tiles
use sl_types::map::{GridCoordinates, ZoomLevel};

/// fetches a map tile from the Second Life main map servers
///
/// # Errors
///
/// returns an error if the HTTP request fails
pub async fn fetch_map_tile(
    client: &reqwest::Client,
    grid_coordinates: &GridCoordinates,
    zoom_level: &ZoomLevel,
) -> Result<reqwest::Response, reqwest::Error> {
    let map_tile_corner = zoom_level.map_tile_corner(grid_coordinates);
    let url = format!(
        "https://secondlife-maps-cdn.akamaized.net/map-{}-{}-{}-objects.jpg",
        zoom_level,
        map_tile_corner.x(),
        map_tile_corner.y()
    );
    client.get(&url).send().await
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_fetch_map_tile_highest_detail() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        fetch_map_tile(
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
        fetch_map_tile(
            &client,
            &GridCoordinates::new(1136, 1075),
            &ZoomLevel::try_new(8)?,
        )
        .await?;
        Ok(())
    }
}
