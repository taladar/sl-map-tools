//! GlobalWind (GLW) data types.
//!
//! All types correspond directly to fields in the JSON shape documented
//! at the top of the repository `TODO.md`. Numeric values are wrapped in
//! validating newtypes so out-of-range data fails at parse time rather
//! than silently producing wrong wind arrows on the map.
//!
//! Direction conventions (verified against the GLW LSL source):
//! - 0° = North, increasing clockwise, in degrees.
//! - Wind direction is the direction the wind blows **from**
//!   (meteorological), so a north wind blows toward the south.
//! - Current direction follows the same convention.

use crate::error::ParseError;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Numeric event identifier assigned by the GLW server.
///
/// Used as the value of the `?id=…` query parameter on the GLW
/// `glwDataReq.php` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventId(u32);

impl EventId {
    /// Wrap a raw numeric id.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }
    /// The underlying numeric id.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::Serialize for EventId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for EventId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(u32::deserialize(deserializer)?))
    }
}

/// String event key (the `eventKey` field of the JSON document).
///
/// The GLW server treats this as opaque text and looks it up via the
/// `?key=…` query parameter on the `glwDataReq.php` endpoint. Named with
/// the `Glw` prefix to avoid colliding with `sl_types::key::EventKey`
/// which represents a Second Life UUID for an event listing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlwEventKey(String);

impl GlwEventKey {
    /// Wrap a raw event-key string.
    #[must_use]
    pub fn new<S: Into<String>>(key: S) -> Self {
        Self(key.into())
    }
    /// Borrow the underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GlwEventKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for GlwEventKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for GlwEventKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(String::deserialize(deserializer)?))
    }
}

// ---------------------------------------------------------------------------
// Validated scalar newtypes
// ---------------------------------------------------------------------------

/// Wind or current direction in degrees from North, increasing clockwise.
///
/// Valid range is `0..=359`. By GLW/meteorological convention, a value
/// of e.g. `180` means the wind is coming **from** the south.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindDirection(u16);

impl WindDirection {
    /// Construct after range validation.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::OutOfRange`] if `degrees` is not in `0..=359`.
    pub fn try_new(degrees: u16) -> Result<Self, ParseError> {
        if degrees > 359 {
            return Err(ParseError::OutOfRange {
                field: "wind_direction",
                value: degrees.to_string(),
                allowed: "0..=359",
            });
        }
        Ok(Self(degrees))
    }
    /// Raw integer degrees in `0..=359`.
    #[must_use]
    pub const fn degrees(self) -> u16 {
        self.0
    }
}

impl serde::Serialize for WindDirection {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for WindDirection {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = u16::deserialize(deserializer)?;
        Self::try_new(v).map_err(serde::de::Error::custom)
    }
}

/// Wind / wave / current speed in knots.
///
/// Stored as `f32`; GLW JSON may use either integer or floating-point
/// representations and serde_json promotes integers transparently. We do
/// not range-validate here because reasonable upper bounds on wind speed
/// are domain-dependent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnotSpeed(f32);

impl KnotSpeed {
    /// Wrap a raw float.
    #[must_use]
    pub const fn new(knots: f32) -> Self {
        Self(knots)
    }
    /// The raw value in knots.
    #[must_use]
    pub const fn knots(self) -> f32 {
        self.0
    }
}

impl serde::Serialize for KnotSpeed {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for KnotSpeed {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(f32::deserialize(deserializer)?))
    }
}

/// Gust intensity as a percentage of base wind speed, valid in `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GustsPercent(u8);

impl GustsPercent {
    /// Construct after range validation.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::OutOfRange`] if `percent` is greater than 100.
    pub fn try_new(percent: u8) -> Result<Self, ParseError> {
        if percent > 100 {
            return Err(ParseError::OutOfRange {
                field: "gusts_percent",
                value: percent.to_string(),
                allowed: "0..=100",
            });
        }
        Ok(Self(percent))
    }
    /// Raw percentage in `0..=100`.
    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }
}

impl serde::Serialize for GustsPercent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for GustsPercent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = u8::deserialize(deserializer)?;
        Self::try_new(v).map_err(serde::de::Error::custom)
    }
}

/// Maximum wind shift in degrees per period (`0..=180`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShiftsDegrees(u16);

impl ShiftsDegrees {
    /// Construct after range validation.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::OutOfRange`] if `degrees` is greater than 180.
    pub fn try_new(degrees: u16) -> Result<Self, ParseError> {
        if degrees > 180 {
            return Err(ParseError::OutOfRange {
                field: "shifts_degrees",
                value: degrees.to_string(),
                allowed: "0..=180",
            });
        }
        Ok(Self(degrees))
    }
    /// Raw degrees in `0..=180`.
    #[must_use]
    pub const fn degrees(self) -> u16 {
        self.0
    }
}

impl serde::Serialize for ShiftsDegrees {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for ShiftsDegrees {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = u16::deserialize(deserializer)?;
        Self::try_new(v).map_err(serde::de::Error::custom)
    }
}

/// Wind shift and gust period in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Period(u32);

impl Period {
    /// Wrap a raw count of seconds.
    #[must_use]
    pub const fn new(seconds: u32) -> Self {
        Self(seconds)
    }
    /// The raw period in seconds.
    #[must_use]
    pub const fn seconds(self) -> u32 {
        self.0
    }
}

impl serde::Serialize for Period {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Period {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(u32::deserialize(deserializer)?))
    }
}

/// Wave height in meters. A value of 0 in JSON is preserved literally
/// (per the GLW spec: "if height=0 there are no waves, but it is set to
/// fill the values of the variations").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveHeight(f32);

impl WaveHeight {
    /// Wrap a raw float.
    #[must_use]
    pub const fn new(meters: f32) -> Self {
        Self(meters)
    }
    /// The raw height in meters.
    #[must_use]
    pub const fn meters(self) -> f32 {
        self.0
    }
}

impl serde::Serialize for WaveHeight {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for WaveHeight {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(f32::deserialize(deserializer)?))
    }
}

/// Wave length in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveLength(f32);

impl WaveLength {
    /// Wrap a raw float.
    #[must_use]
    pub const fn new(meters: f32) -> Self {
        Self(meters)
    }
    /// The raw length in meters.
    #[must_use]
    pub const fn meters(self) -> f32 {
        self.0
    }
}

impl serde::Serialize for WaveLength {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for WaveLength {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(f32::deserialize(deserializer)?))
    }
}

/// Percentage variance applied to a wave height/length (`0..=100`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PercentVariance(u8);

impl PercentVariance {
    /// Construct after range validation.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::OutOfRange`] if `percent` is greater than 100.
    pub fn try_new(percent: u8) -> Result<Self, ParseError> {
        if percent > 100 {
            return Err(ParseError::OutOfRange {
                field: "percent_variance",
                value: percent.to_string(),
                allowed: "0..=100",
            });
        }
        Ok(Self(percent))
    }
    /// Raw percent in `0..=100`.
    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }
}

impl serde::Serialize for PercentVariance {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for PercentVariance {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = u8::deserialize(deserializer)?;
        Self::try_new(v).map_err(serde::de::Error::custom)
    }
}

/// Two boolean toggles describing how waves affect a boat: whether they
/// modulate its speed and whether they perturb its heading.
///
/// In GLW JSON the field appears either as an object
/// `{"speed": 0|1, "steer": 0|1}` or as an integer bitmask
/// (`0 = none`, `1 = steer`, `2 = speed`, `3 = both`). Both forms are
/// accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveEffects {
    /// Whether waves modulate the boat's forward speed.
    pub speed: bool,
    /// Whether waves perturb the boat's heading.
    pub steer: bool,
}

impl serde::Serialize for WaveEffects {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct as _;
        let mut s = serializer.serialize_struct("WaveEffects", 2)?;
        s.serialize_field("speed", &u8::from(self.speed))?;
        s.serialize_field("steer", &u8::from(self.steer))?;
        s.end()
    }
}

impl<'de> serde::Deserialize<'de> for WaveEffects {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Internal helper to deserialize either of the two JSON shapes.
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Raw {
            /// JSON integer 0..=3, treated as a bitmask
            /// (bit 0 = steer, bit 1 = speed) per the GLW LSL convention.
            Bitmask(u8),
            /// JSON object with `speed` and `steer` ints (0 or 1).
            Object {
                /// `1` if the speed effect is on, `0` otherwise.
                #[serde(default)]
                speed: u8,
                /// `1` if the steer effect is on, `0` otherwise.
                #[serde(default)]
                steer: u8,
            },
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(match raw {
            Raw::Bitmask(n) => Self {
                speed: (n & 0b10) != 0,
                steer: (n & 0b01) != 0,
            },
            Raw::Object { speed, steer } => Self {
                speed: speed != 0,
                steer: steer != 0,
            },
        })
    }
}

/// A strictly positive water depth in meters.
///
/// The GLW JSON uses `0` to mean "depth-attenuation disabled"; this
/// crate models that as the absence of a value (see
/// [`WaterDepthSetting`]) instead of allowing a zero value here. Negative
/// values are rejected at parse time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterDepth(f32);

impl WaterDepth {
    /// Construct after validation.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::OutOfRange`] if `meters <= 0.0`.
    pub fn try_new(meters: f32) -> Result<Self, ParseError> {
        if meters > 0.0 {
            Ok(Self(meters))
        } else {
            Err(ParseError::OutOfRange {
                field: "water_depth",
                value: meters.to_string(),
                allowed: "> 0",
            })
        }
    }
    /// The raw depth in meters (always strictly positive).
    #[must_use]
    pub const fn meters(self) -> f32 {
        self.0
    }
}

impl serde::Serialize for WaterDepth {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.0)
    }
}

/// Tri-state setting for water depth on a `BaseCurrents` block.
///
/// JSON value 0 maps to [`Self::Disabled`]. Positive values map to
/// [`Self::Set`]. Negative values are rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaterDepthSetting {
    /// Depth attenuation is disabled (JSON `0`).
    Disabled,
    /// A positive depth in meters.
    Set(WaterDepth),
}

impl serde::Serialize for WaterDepthSetting {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::Disabled => serializer.serialize_f32(0.0),
            Self::Set(d) => serializer.serialize_f32(d.meters()),
        }
    }
}

impl<'de> serde::Deserialize<'de> for WaterDepthSetting {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f32::deserialize(deserializer)?;
        // exact 0.0 is the GLW disabled sentinel; serde_json maps the
        // integer 0 to exactly 0.0_f32 so the comparison is well-defined.
        let is_zero = v == 0.0;
        if is_zero {
            Ok(Self::Disabled)
        } else {
            WaterDepth::try_new(v)
                .map(Self::Set)
                .map_err(serde::de::Error::custom)
        }
    }
}

/// Per-area or per-circle margin in meters, clamped to `0..=100`.
///
/// The GLW spec documents a maximum of 100 m; values above 100 are
/// clamped to 100 and emit a `tracing::warn!` rather than failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MarginMeters(u8);

impl MarginMeters {
    /// Construct, clamping values above 100 (with a `tracing::warn!`).
    #[must_use]
    pub fn new_clamped(meters: u16) -> Self {
        if meters > 100 {
            tracing::warn!(
                requested = meters,
                clamped = 100u8,
                "GLW margin exceeds documented maximum (100m); clamping",
            );
            Self(100)
        } else {
            // safe: meters <= 100 fits in u8
            Self(u8::try_from(meters).unwrap_or(100))
        }
    }
    /// Raw margin in meters (`0..=100`).
    #[must_use]
    pub const fn meters(self) -> u8 {
        self.0
    }
}

impl serde::Serialize for MarginMeters {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for MarginMeters {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::new_clamped(u16::deserialize(deserializer)?))
    }
}

/// Radius of a GLW circle in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusMeters(f32);

impl RadiusMeters {
    /// Construct after non-negative validation.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::OutOfRange`] if `meters < 0.0`.
    pub fn try_new(meters: f32) -> Result<Self, ParseError> {
        if meters < 0.0 {
            return Err(ParseError::OutOfRange {
                field: "radius_meters",
                value: meters.to_string(),
                allowed: ">= 0",
            });
        }
        Ok(Self(meters))
    }
    /// Raw radius in meters.
    #[must_use]
    pub const fn meters(self) -> f32 {
        self.0
    }
}

impl serde::Serialize for RadiusMeters {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for RadiusMeters {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f32::deserialize(deserializer)?;
        Self::try_new(v).map_err(serde::de::Error::custom)
    }
}

/// Whether an area or circle is a nested override on top of another
/// area or circle (`overlap = 1`) rather than directly modifying the
/// base wind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Overlap(bool);

impl Overlap {
    /// Wrap a raw bool.
    #[must_use]
    pub const fn new(v: bool) -> Self {
        Self(v)
    }
    /// The raw bool value.
    #[must_use]
    pub const fn get(self) -> bool {
        self.0
    }
}

impl serde::Serialize for Overlap {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(u8::from(self.0))
    }
}

impl<'de> serde::Deserialize<'de> for Overlap {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(u8::deserialize(deserializer)? != 0))
    }
}

// ---------------------------------------------------------------------------
// Base wind / waves / currents (always fully populated)
// ---------------------------------------------------------------------------

/// Base wind parameters that apply to the whole event unless an area or
/// circle override is in effect.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BaseWind {
    /// Direction the wind blows **from**, in degrees clockwise from North.
    #[serde(rename = "dir")]
    pub direction: WindDirection,
    /// Base wind speed in knots.
    pub speed: KnotSpeed,
    /// Gust intensity as a percentage of base wind speed.
    pub gusts: GustsPercent,
    /// Maximum wind shift in degrees per period.
    pub shifts: ShiftsDegrees,
    /// Period of shift/gust oscillation in seconds.
    pub period: Period,
}

/// Base wave parameters that apply to the whole event unless an area or
/// circle override is in effect.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseWaves {
    /// Wave height in meters. A height of zero means "no waves", though
    /// other fields may still carry meaningful variance values.
    pub height: WaveHeight,
    /// Wave speed in knots.
    pub speed: KnotSpeed,
    /// Wave length in meters.
    pub length: WaveLength,
    /// Whether waves affect boat speed and/or steering.
    pub effects: WaveEffects,
    /// Variance applied to wave height, as a percentage.
    pub height_var: PercentVariance,
    /// Variance applied to wave length, as a percentage.
    pub length_var: PercentVariance,
}

/// Base current parameters that apply to the whole event unless an area
/// or circle override is in effect.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseCurrents {
    /// Current speed in knots. Zero means no current; the other fields
    /// still carry meaningful variance values.
    pub speed: KnotSpeed,
    /// Direction the current flows **from**, in degrees clockwise from
    /// North.
    #[serde(rename = "dir")]
    pub direction: WindDirection,
    /// Water-depth setting controlling how currents attenuate with
    /// real-world sea depth.
    pub water_depth: WaterDepthSetting,
}

/// The full base block: wind, waves, currents combined.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Base {
    /// Base wind parameters.
    pub wind: BaseWind,
    /// Base wave parameters.
    pub waves: BaseWaves,
    /// Base current parameters.
    pub currents: BaseCurrents,
}

// ---------------------------------------------------------------------------
// Per-area / per-circle overrides (every field optional)
// ---------------------------------------------------------------------------

/// Wind override block. Each field is independently optional: only the
/// fields actually overridden by the area/circle are present.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WindOverride {
    /// Direction the wind blows from, in degrees clockwise from North.
    #[serde(rename = "dir", skip_serializing_if = "Option::is_none")]
    pub direction: Option<WindDirection>,
    /// Wind speed in knots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<KnotSpeed>,
    /// Gust intensity (`0..=100`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gusts: Option<GustsPercent>,
    /// Maximum wind shift in degrees per period.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shifts: Option<ShiftsDegrees>,
    /// Period of shift/gust oscillation in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
}

impl WindOverride {
    /// `true` if every field is `None`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.direction.is_none()
            && self.speed.is_none()
            && self.gusts.is_none()
            && self.shifts.is_none()
            && self.period.is_none()
    }
}

/// Wave override block. Each field is independently optional.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WavesOverride {
    /// Wave height in meters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<WaveHeight>,
    /// Wave speed in knots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<KnotSpeed>,
    /// Wave length in meters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<WaveLength>,
    /// Whether waves affect boat speed and/or steering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<WaveEffects>,
    /// Variance applied to wave height, as a percentage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_var: Option<PercentVariance>,
    /// Variance applied to wave length, as a percentage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_var: Option<PercentVariance>,
}

impl WavesOverride {
    /// `true` if every field is `None`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.height.is_none()
            && self.speed.is_none()
            && self.length.is_none()
            && self.effects.is_none()
            && self.height_var.is_none()
            && self.length_var.is_none()
    }
}

/// Currents override block. Each field is independently optional.
///
/// Note `water_depth: Option<WaterDepthSetting>`: outer `None` means the
/// override does not touch water depth at all; inner
/// [`WaterDepthSetting::Disabled`] means the override explicitly
/// disables depth attenuation.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CurrentsOverride {
    /// Current speed in knots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<KnotSpeed>,
    /// Direction the current flows from, in degrees clockwise from North.
    #[serde(rename = "dir", skip_serializing_if = "Option::is_none")]
    pub direction: Option<WindDirection>,
    /// Water-depth setting controlling how currents attenuate with
    /// real-world sea depth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub water_depth: Option<WaterDepthSetting>,
}

impl CurrentsOverride {
    /// `true` if every field is `None`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.speed.is_none() && self.direction.is_none() && self.water_depth.is_none()
    }
}

// ---------------------------------------------------------------------------
// Area / Circle / Event
// ---------------------------------------------------------------------------

/// A rectangular GLW area covering one or more whole regions.
///
/// `name` is the original JSON key (e.g. `"area1"`). Document order is
/// preserved by [`AreaList`] so that `overlap = true` layering matches
/// the GLW LSL/PHP reference implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    /// JSON key for this area (e.g. `"area1"`).
    pub name: String,
    /// Region rectangle covered by this area.
    pub grid_rectangle: sl_types::map::GridRectangle,
    /// Margin width in meters where conditions blend with the base.
    pub margin: MarginMeters,
    /// `true` when this area overlays another area/circle rather than the
    /// base wind directly.
    pub overlap: Overlap,
    /// Optional wind override.
    pub wind: Option<WindOverride>,
    /// Optional wave override.
    pub waves: Option<WavesOverride>,
    /// Optional currents override.
    pub currents: Option<CurrentsOverride>,
}

/// A circular GLW area.
///
/// `name` is the original JSON key (e.g. `"circle1"`).
#[derive(Debug, Clone, PartialEq)]
pub struct Circle {
    /// JSON key for this circle (e.g. `"circle1"`).
    pub name: String,
    /// Region containing the centre point.
    pub center_sim: sl_types::map::GridCoordinates,
    /// Coordinates of the centre within `center_sim`, in meters from the
    /// SW corner of the region.
    pub center_point: sl_types::map::RegionCoordinates,
    /// Radius of the circle in meters.
    pub radius: RadiusMeters,
    /// Margin width in meters where conditions blend with the base.
    pub margin: MarginMeters,
    /// `true` when this circle overlays another area/circle rather than
    /// the base wind directly.
    pub overlap: Overlap,
    /// Optional wind override.
    pub wind: Option<WindOverride>,
    /// Optional wave override.
    pub waves: Option<WavesOverride>,
    /// Optional currents override.
    pub currents: Option<CurrentsOverride>,
}

/// Document-ordered list of [`Area`]s.
///
/// Order matters: overlap layering applies in document order. Serializes
/// back as a JSON object keyed by `Area::name` so it round-trips against
/// the original GLW JSON shape.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AreaList(pub Vec<Area>);

impl AreaList {
    /// Borrow the underlying `Vec`.
    #[must_use]
    pub fn as_slice(&self) -> &[Area] {
        &self.0
    }
    /// Number of areas in the list.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
    /// `true` when the list contains no areas.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl serde::Serialize for AreaList {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for area in &self.0 {
            map.serialize_entry(&area.name, &AreaSerializable::from(area))?;
        }
        map.end()
    }
}

/// Private view of an [`Area`] without its `name` field, used only for
/// serialization so the area's body matches the GLW JSON shape (the
/// name is the JSON key, not a body field).
#[derive(serde::Serialize)]
struct AreaSerializable<'a> {
    /// SW corner (sim coordinates).
    #[serde(rename = "coordSW")]
    coord_sw: SimCoord,
    /// NE corner (sim coordinates).
    #[serde(rename = "coordNE")]
    coord_ne: SimCoord,
    /// Margin width in meters.
    margin: &'a MarginMeters,
    /// Whether this area overlays another area/circle rather than the
    /// base wind directly.
    overlap: &'a Overlap,
    /// Wind override block.
    #[serde(skip_serializing_if = "Option::is_none")]
    wind: Option<&'a WindOverride>,
    /// Wave override block.
    #[serde(skip_serializing_if = "Option::is_none")]
    waves: Option<&'a WavesOverride>,
    /// Currents override block.
    #[serde(skip_serializing_if = "Option::is_none")]
    currents: Option<&'a CurrentsOverride>,
}

impl<'a> From<&'a Area> for AreaSerializable<'a> {
    fn from(a: &'a Area) -> Self {
        Self {
            coord_sw: SimCoord::from_grid(&a.grid_rectangle, GridCorner::Sw),
            coord_ne: SimCoord::from_grid(&a.grid_rectangle, GridCorner::Ne),
            margin: &a.margin,
            overlap: &a.overlap,
            wind: a.wind.as_ref(),
            waves: a.waves.as_ref(),
            currents: a.currents.as_ref(),
        }
    }
}

/// Document-ordered list of [`Circle`]s.
///
/// Order matters: overlap layering applies in document order. Serializes
/// back as a JSON object keyed by `Circle::name` so it round-trips
/// against the original GLW JSON shape.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CircleList(pub Vec<Circle>);

impl CircleList {
    /// Borrow the underlying `Vec`.
    #[must_use]
    pub fn as_slice(&self) -> &[Circle] {
        &self.0
    }
    /// Number of circles in the list.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
    /// `true` when the list contains no circles.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl serde::Serialize for CircleList {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for circle in &self.0 {
            map.serialize_entry(&circle.name, &CircleSerializable::from(circle))?;
        }
        map.end()
    }
}

/// Private view of a [`Circle`] without its `name` field, used only for
/// serialization.
#[derive(serde::Serialize)]
struct CircleSerializable<'a> {
    /// Sim containing the circle centre.
    #[serde(rename = "centerSim")]
    center_sim: SimCoord,
    /// Coordinates of the centre within `centerSim`.
    #[serde(rename = "centerPoint")]
    center_point: RegionXY,
    /// Circle radius in meters.
    radius: &'a RadiusMeters,
    /// Margin width in meters.
    margin: &'a MarginMeters,
    /// Whether this circle overlays another area/circle.
    overlap: &'a Overlap,
    /// Wind override block.
    #[serde(skip_serializing_if = "Option::is_none")]
    wind: Option<&'a WindOverride>,
    /// Wave override block.
    #[serde(skip_serializing_if = "Option::is_none")]
    waves: Option<&'a WavesOverride>,
    /// Currents override block.
    #[serde(skip_serializing_if = "Option::is_none")]
    currents: Option<&'a CurrentsOverride>,
}

impl<'a> From<&'a Circle> for CircleSerializable<'a> {
    fn from(c: &'a Circle) -> Self {
        Self {
            center_sim: SimCoord {
                x: c.center_sim.x(),
                y: c.center_sim.y(),
            },
            center_point: RegionXY {
                x: c.center_point.x(),
                y: c.center_point.y(),
            },
            radius: &c.radius,
            margin: &c.margin,
            overlap: &c.overlap,
            wind: c.wind.as_ref(),
            waves: c.waves.as_ref(),
            currents: c.currents.as_ref(),
        }
    }
}

/// Helper enum used by [`SimCoord::from_grid`] to pick a corner.
#[derive(Debug, Clone, Copy)]
enum GridCorner {
    /// South-west (lower-left) corner.
    Sw,
    /// North-east (upper-right) corner.
    Ne,
}

/// Serialization-side `{ "x": u16, "y": u16 }` shape (mirrors the
/// `GridCoordRaw` in `parse.rs`).
#[derive(Debug, Clone, Copy, serde::Serialize)]
struct SimCoord {
    /// Sim x grid coordinate.
    x: u32,
    /// Sim y grid coordinate.
    y: u32,
}

impl SimCoord {
    /// Project one corner of a [`sl_types::map::GridRectangle`] into the
    /// flat `{x,y}` shape used in JSON.
    fn from_grid(rect: &sl_types::map::GridRectangle, which: GridCorner) -> Self {
        use sl_types::map::GridRectangleLike as _;
        let gc = match which {
            GridCorner::Sw => rect.lower_left_corner(),
            GridCorner::Ne => rect.upper_right_corner(),
        };
        Self {
            x: gc.x(),
            y: gc.y(),
        }
    }
}

/// Serialization-side `{ "x": f32, "y": f32 }` shape for circle
/// centerPoints (z is intentionally dropped to match GLW JSON, which
/// only carries x and y).
#[derive(Debug, Clone, Copy, serde::Serialize)]
struct RegionXY {
    /// X meters from the western edge of the sim.
    x: f32,
    /// Y meters from the southern edge of the sim.
    y: f32,
}

/// A full GLW event as returned by `glwDataReq.php`.
///
/// Unknown top-level fields are silently ignored — the GLW author may
/// extend the schema, and unknown fields do not break parsing.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlwEvent {
    /// Numeric event id assigned by the server.
    pub event_id: EventId,
    /// Free-text event name (e.g. `"TYC Cruise"`).
    pub event_name: String,
    /// Opaque event key string.
    pub event_key: GlwEventKey,
    /// Free-form event number (informational; not used by GLW).
    #[serde(default)]
    pub event_num: Option<u32>,
    /// Display name of the event director.
    pub director_name: String,
    /// Second Life UUID of the event director (stored as a string; this
    /// crate does not require the `uuid` dependency).
    pub director_key: String,
    /// Sail-mode flag. Documented as "not use for now"; kept optional
    /// for forward compatibility.
    #[serde(default)]
    pub sail_mode: Option<u8>,
    /// Opaque string field forwarded to boats (max 15 chars in the
    /// reference schema; not validated here).
    #[serde(default)]
    pub extra1: String,
    /// Opaque string field forwarded to boats (max 15 chars in the
    /// reference schema; not validated here).
    #[serde(default)]
    pub extra2: String,
    /// Base wind/waves/currents that apply unless overridden.
    pub base: Base,
    /// Per-area overrides, in document order. JSON key names are
    /// preserved on each [`Area`].
    #[serde(default, deserialize_with = "crate::parse::deserialize_area_list")]
    pub areas: AreaList,
    /// Per-circle overrides, in document order.
    #[serde(default, deserialize_with = "crate::parse::deserialize_circle_list")]
    pub circles: CircleList,
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn wind_direction_rejects_out_of_range() {
        assert!(WindDirection::try_new(0).is_ok(), "0 is in range");
        assert!(WindDirection::try_new(359).is_ok(), "359 is in range");
        assert!(WindDirection::try_new(360).is_err(), "360 is out of range");
        assert!(WindDirection::try_new(999).is_err(), "999 is out of range");
    }

    #[test]
    fn gusts_rejects_out_of_range() {
        assert!(GustsPercent::try_new(0).is_ok(), "0 is in range");
        assert!(GustsPercent::try_new(100).is_ok(), "100 is in range");
        assert!(GustsPercent::try_new(101).is_err(), "101 is out of range");
    }

    #[test]
    fn shifts_rejects_out_of_range() {
        assert!(ShiftsDegrees::try_new(180).is_ok(), "180 is in range");
        assert!(ShiftsDegrees::try_new(181).is_err(), "181 is out of range");
    }

    #[test]
    fn margin_clamps_at_100() {
        assert_eq!(MarginMeters::new_clamped(0).meters(), 0);
        assert_eq!(MarginMeters::new_clamped(100).meters(), 100);
        assert_eq!(MarginMeters::new_clamped(150).meters(), 100);
    }

    #[test]
    fn water_depth_rejects_zero_and_negative() {
        assert!(WaterDepth::try_new(1.0).is_ok(), "1.0 is positive");
        assert!(WaterDepth::try_new(0.0).is_err(), "0.0 is rejected");
        assert!(WaterDepth::try_new(-0.5).is_err(), "negative rejected");
    }

    #[test]
    fn water_depth_setting_maps_zero_to_disabled() -> Result<(), Box<dyn std::error::Error>> {
        let disabled: WaterDepthSetting = serde_json::from_str("0")?;
        assert_eq!(disabled, WaterDepthSetting::Disabled);
        let positive: WaterDepthSetting = serde_json::from_str("6.5")?;
        match positive {
            WaterDepthSetting::Set(d) => {
                assert!((d.meters() - 6.5).abs() < 1e-6, "6.5 round-trips");
            }
            WaterDepthSetting::Disabled => {
                return Err("6.5 must not become Disabled".into());
            }
        }
        let negative: Result<WaterDepthSetting, _> = serde_json::from_str("-1");
        assert!(negative.is_err(), "negative rejected");
        Ok(())
    }

    #[test]
    fn overlap_from_0_and_1() -> Result<(), Box<dyn std::error::Error>> {
        let off: Overlap = serde_json::from_str("0")?;
        assert_eq!(off, Overlap::new(false));
        let on: Overlap = serde_json::from_str("1")?;
        assert_eq!(on, Overlap::new(true));
        Ok(())
    }

    #[test]
    fn wave_effects_from_object() -> Result<(), Box<dyn std::error::Error>> {
        let we: WaveEffects = serde_json::from_str(r#"{"speed":1,"steer":0}"#)?;
        assert!(we.speed, "speed=1 turns on");
        assert!(!we.steer, "steer=0 stays off");
        Ok(())
    }

    #[test]
    fn wave_effects_from_bitmask() -> Result<(), Box<dyn std::error::Error>> {
        let we: WaveEffects = serde_json::from_str("3")?;
        assert!(we.speed && we.steer, "bitmask 3 = both on");
        let we: WaveEffects = serde_json::from_str("2")?;
        assert!(we.speed && !we.steer, "bitmask 2 = speed only");
        let we: WaveEffects = serde_json::from_str("1")?;
        assert!(!we.speed && we.steer, "bitmask 1 = steer only");
        let we: WaveEffects = serde_json::from_str("0")?;
        assert!(!we.speed && !we.steer, "bitmask 0 = neither");
        Ok(())
    }
}
