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
}
