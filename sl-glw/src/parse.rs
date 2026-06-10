//! Custom deserializers for the keyed-object `areas` and `circles`
//! collections.
//!
//! GLW expresses both as JSON objects keyed by `area1`/`area2`/... or
//! `circle1`/`circle2`/... rather than as arrays. The keys carry no
//! semantic meaning beyond identifying the override, but document order
//! is significant for `overlap` layering. We therefore walk the object
//! with a [`serde::de::MapAccess`] visitor that preserves insertion
//! order in a `Vec`.

use crate::types::{
    Area, AreaList, Circle, CircleList, CurrentsOverride, MarginMeters, Overlap, RadiusMeters,
    WavesOverride, WindOverride,
};

/// Raw `{ "x": u16, "y": u16 }` pair used for `coordSW`, `coordNE`,
/// `centerSim` etc.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct GridCoordRaw {
    /// Sim x grid coordinate.
    x: u16,
    /// Sim y grid coordinate.
    y: u16,
}

/// Raw `{ "x": f32, "y": f32 }` pair used for the `centerPoint` of a
/// circle (meters from the SW corner of the containing sim).
#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct RegionCoordRaw {
    /// X meters from the western edge of the sim.
    x: f32,
    /// Y meters from the southern edge of the sim.
    y: f32,
}

/// Raw JSON shape for an entry of the `areas` object, before
/// post-processing into a typed [`Area`].
#[derive(Debug, Clone, serde::Deserialize)]
struct AreaRaw {
    /// SW corner of the rectangle (sim coordinates).
    #[serde(rename = "coordSW")]
    coord_sw: GridCoordRaw,
    /// NE corner of the rectangle (sim coordinates).
    #[serde(rename = "coordNE")]
    coord_ne: GridCoordRaw,
    /// Margin width in meters.
    margin: MarginMeters,
    /// Whether this area overlays another area/circle rather than the
    /// base wind directly.
    overlap: Overlap,
    /// Wind override block (may be absent or have all fields absent).
    #[serde(default)]
    wind: Option<WindOverride>,
    /// Wave override block (may be absent).
    #[serde(default)]
    waves: Option<WavesOverride>,
    /// Currents override block (may be absent).
    #[serde(default)]
    currents: Option<CurrentsOverride>,
}

/// Raw JSON shape for an entry of the `circles` object, before
/// post-processing into a typed [`Circle`].
#[derive(Debug, Clone, serde::Deserialize)]
struct CircleRaw {
    /// Sim containing the circle centre.
    #[serde(rename = "centerSim")]
    center_sim: GridCoordRaw,
    /// Position of the circle centre within the containing sim.
    #[serde(rename = "centerPoint")]
    center_point: RegionCoordRaw,
    /// Circle radius in meters.
    radius: RadiusMeters,
    /// Margin width in meters.
    margin: MarginMeters,
    /// Whether this circle overlays another area/circle rather than the
    /// base wind directly.
    overlap: Overlap,
    /// Wind override block (may be absent or empty).
    #[serde(default)]
    wind: Option<WindOverride>,
    /// Wave override block (may be absent).
    #[serde(default)]
    waves: Option<WavesOverride>,
    /// Currents override block (may be absent).
    #[serde(default)]
    currents: Option<CurrentsOverride>,
}

/// Collapse a `Some(empty)` override into `None` so consumers can use a
/// simple `is_some()` check to mean "this area overrides at least one
/// field of this category".
fn normalize_wind(o: Option<WindOverride>) -> Option<WindOverride> {
    o.filter(|w| !w.is_empty())
}

/// Same as [`normalize_wind`] for wave overrides.
fn normalize_waves(o: Option<WavesOverride>) -> Option<WavesOverride> {
    o.filter(|w| !w.is_empty())
}

/// Same as [`normalize_wind`] for current overrides.
fn normalize_currents(o: Option<CurrentsOverride>) -> Option<CurrentsOverride> {
    o.filter(|c| !c.is_empty())
}

/// Build a typed [`Area`] from a raw entry plus its original JSON key
/// name (preserved for round-tripping and labeling).
fn area_from_raw(name: String, raw: AreaRaw) -> Area {
    Area {
        grid_rectangle: sl_types::map::GridRectangle::new(
            sl_types::map::GridCoordinates::new(raw.coord_sw.x, raw.coord_sw.y),
            sl_types::map::GridCoordinates::new(raw.coord_ne.x, raw.coord_ne.y),
        ),
        name,
        margin: raw.margin,
        overlap: raw.overlap,
        wind: normalize_wind(raw.wind),
        waves: normalize_waves(raw.waves),
        currents: normalize_currents(raw.currents),
    }
}

/// Build a typed [`Circle`] from a raw entry plus its original JSON key.
fn circle_from_raw(name: String, raw: CircleRaw) -> Circle {
    Circle {
        center_sim: sl_types::map::GridCoordinates::new(raw.center_sim.x, raw.center_sim.y),
        center_point: sl_types::map::RegionCoordinates::new(
            raw.center_point.x,
            raw.center_point.y,
            0.0,
        ),
        name,
        radius: raw.radius,
        margin: raw.margin,
        overlap: raw.overlap,
        wind: normalize_wind(raw.wind),
        waves: normalize_waves(raw.waves),
        currents: normalize_currents(raw.currents),
    }
}

/// Deserialize the `areas` object into an order-preserving `Vec`.
///
/// # Errors
///
/// Forwarded from the underlying serde deserializer.
pub(crate) fn deserialize_area_list<'de, D>(de: D) -> Result<AreaList, D::Error>
where
    D: serde::Deserializer<'de>,
{
    /// `serde::de::Visitor` that builds an [`AreaList`] from a JSON object.
    struct AreaListVisitor;
    impl<'de> serde::de::Visitor<'de> for AreaListVisitor {
        type Value = AreaList;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON object whose keys are area names (e.g. \"area1\")")
        }
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut areas: Vec<Area> = match map.size_hint() {
                Some(n) => Vec::with_capacity(n),
                None => Vec::new(),
            };
            while let Some((name, raw)) = map.next_entry::<String, AreaRaw>()? {
                areas.push(area_from_raw(name, raw));
            }
            Ok(AreaList(areas))
        }
    }
    de.deserialize_map(AreaListVisitor)
}

/// Deserialize the `circles` object into an order-preserving `Vec`.
///
/// # Errors
///
/// Forwarded from the underlying serde deserializer.
pub(crate) fn deserialize_circle_list<'de, D>(de: D) -> Result<CircleList, D::Error>
where
    D: serde::Deserializer<'de>,
{
    /// `serde::de::Visitor` that builds a [`CircleList`] from a JSON object.
    struct CircleListVisitor;
    impl<'de> serde::de::Visitor<'de> for CircleListVisitor {
        type Value = CircleList;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON object whose keys are circle names (e.g. \"circle1\")")
        }
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut circles: Vec<Circle> = match map.size_hint() {
                Some(n) => Vec::with_capacity(n),
                None => Vec::new(),
            };
            while let Some((name, raw)) = map.next_entry::<String, CircleRaw>()? {
                circles.push(circle_from_raw(name, raw));
            }
            Ok(CircleList(circles))
        }
    }
    de.deserialize_map(CircleListVisitor)
}

#[cfg(test)]
mod test {
    use crate::types::GlwEvent;
    use pretty_assertions::assert_eq;

    /// Minimal but complete sample JSON that exercises the keyed-object
    /// areas/circles deserializer. Trimmed from the document at the top
    /// of `TODO.md`.
    const SAMPLE_JSON: &str = r#"{
        "eventId": 6910,
        "eventName": "test cruise",
        "eventKey": "key cruise",
        "eventNum": 3381,
        "directorName": "LaliaCasau Resident",
        "directorKey": "b609826a-b167-41e0-8e67-9fc0e78b97a1",
        "sailMode": 2,
        "extra1": "",
        "extra2": "",
        "base": {
            "wind": { "dir": 175, "speed": 17, "gusts": 8, "shifts": 5, "period": 90 },
            "waves": {
                "height": 1.5, "speed": 3, "length": 35,
                "heightVar": 5, "lengthVar": 5,
                "effects": { "speed": 1, "steer": 1 }
            },
            "currents": { "speed": 0, "dir": 180, "waterDepth": 0 }
        },
        "areas": {
            "area1": {
                "coordSW": { "x": 1133, "y": 1048 },
                "coordNE": { "x": 1135, "y": 1049 },
                "margin": 25, "overlap": 0,
                "currents": { "speed": 1, "dir": 225, "waterDepth": 8 }
            },
            "area2": {
                "coordSW": { "x": 1133, "y": 1050 },
                "coordNE": { "x": 1134, "y": 1053 },
                "margin": 0, "overlap": 0,
                "currents": { "speed": 1, "dir": 250 }
            }
        },
        "circles": {
            "circle1": {
                "centerSim": { "x": 1136, "y": 1051 },
                "centerPoint": { "x": 90, "y": 175 },
                "radius": 127, "margin": 25, "overlap": 0,
                "wind": { "speed": 15 },
                "waves": {
                    "height": 2, "speed": 3, "length": 35,
                    "heightVar": 5, "lengthVar": 5,
                    "effects": { "speed": 1, "steer": 1 }
                },
                "currents": { "speed": 0.1, "dir": 225, "waterDepth": 6 }
            },
            "circle2": {
                "centerSim": { "x": 1139, "y": 1051 },
                "centerPoint": { "x": 12, "y": 159 },
                "radius": 261, "margin": 25, "overlap": 0,
                "currents": { "speed": 2, "dir": 90, "waterDepth": 6 }
            }
        }
    }"#;

    #[test]
    fn parses_full_sample() -> Result<(), Box<dyn std::error::Error>> {
        let evt: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;
        assert_eq!(evt.event_id.get(), 6910);
        assert_eq!(evt.event_name, "test cruise");
        assert_eq!(evt.event_key.as_str(), "key cruise");
        assert_eq!(evt.event_num, Some(3381));
        assert_eq!(evt.director_name, "LaliaCasau Resident");
        assert_eq!(evt.sail_mode, Some(2));
        assert_eq!(evt.areas.len(), 2);
        assert_eq!(evt.circles.len(), 2);
        Ok(())
    }

    #[test]
    fn area_order_is_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let evt: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;
        let names: Vec<&str> = evt
            .areas
            .as_slice()
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(names, vec!["area1", "area2"]);
        Ok(())
    }

    #[test]
    fn circle_order_is_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let evt: GlwEvent = serde_json::from_str(SAMPLE_JSON)?;
        let names: Vec<&str> = evt
            .circles
            .as_slice()
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, vec!["circle1", "circle2"]);
        Ok(())
    }

    #[test]
    fn empty_areas_and_circles_default() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "eventId": 1,
            "eventName": "x",
            "eventKey": "k",
            "directorName": "D",
            "directorKey": "00000000-0000-0000-0000-000000000000",
            "base": {
                "wind": { "dir": 0, "speed": 0, "gusts": 0, "shifts": 0, "period": 0 },
                "waves": {
                    "height": 0, "speed": 0, "length": 0,
                    "heightVar": 0, "lengthVar": 0,
                    "effects": { "speed": 0, "steer": 0 }
                },
                "currents": { "speed": 0, "dir": 0, "waterDepth": 0 }
            }
        }"#;
        let evt: GlwEvent = serde_json::from_str(json)?;
        assert!(evt.areas.is_empty());
        assert!(evt.circles.is_empty());
        Ok(())
    }

    #[test]
    fn out_of_range_field_fails_parse() {
        let json = r#"{
            "eventId": 1,
            "eventName": "x",
            "eventKey": "k",
            "directorName": "D",
            "directorKey": "00000000-0000-0000-0000-000000000000",
            "base": {
                "wind": { "dir": 999, "speed": 0, "gusts": 0, "shifts": 0, "period": 0 },
                "waves": {
                    "height": 0, "speed": 0, "length": 0,
                    "heightVar": 0, "lengthVar": 0,
                    "effects": 0
                },
                "currents": { "speed": 0, "dir": 0, "waterDepth": 0 }
            }
        }"#;
        let result: Result<GlwEvent, _> = serde_json::from_str(json);
        assert!(result.is_err(), "wind dir 999 should be rejected");
    }

    #[test]
    fn area_with_no_overrides_yields_none() -> Result<(), Box<dyn std::error::Error>> {
        // Per the GLW spec only fields actually overridden appear; this
        // test exercises the absence of all three override blocks.
        let json = r#"{
            "eventId": 1,
            "eventName": "x",
            "eventKey": "k",
            "directorName": "D",
            "directorKey": "00000000-0000-0000-0000-000000000000",
            "base": {
                "wind": { "dir": 0, "speed": 0, "gusts": 0, "shifts": 0, "period": 0 },
                "waves": {
                    "height": 0, "speed": 0, "length": 0,
                    "heightVar": 0, "lengthVar": 0,
                    "effects": 0
                },
                "currents": { "speed": 0, "dir": 0, "waterDepth": 0 }
            },
            "areas": {
                "area1": {
                    "coordSW": { "x": 1000, "y": 1000 },
                    "coordNE": { "x": 1001, "y": 1001 },
                    "margin": 0,
                    "overlap": 0
                }
            }
        }"#;
        let evt: GlwEvent = serde_json::from_str(json)?;
        let area = evt
            .areas
            .as_slice()
            .first()
            .ok_or("area1 must be present")?;
        assert!(area.wind.is_none(), "wind override absent");
        assert!(area.waves.is_none(), "waves override absent");
        assert!(area.currents.is_none(), "currents override absent");
        Ok(())
    }
}
