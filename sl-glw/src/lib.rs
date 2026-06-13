#![doc = include_str!("../README.md")]

pub mod cache;
pub mod client;
pub mod error;
pub(crate) mod geometry;
pub(crate) mod parse;
pub mod render;
pub mod style;
pub mod types;

pub use crate::cache::GlwEventCache;
pub use crate::client::{
    DEFAULT_GLW_HOST, DEFAULT_GLW_VERSION, GlwClient, base_url_for_version, default_base_url,
    fetch_event_by_id, fetch_event_by_key,
};
pub use crate::error::{Error, FetchError, GlwEventCacheError, ParseError};
pub use crate::render::{MapLikeGlwExt, draw_glw_event};
pub use crate::style::{GlwColorPalette, GlwStyle};
pub use crate::types::{
    Area, AreaList, Base, BaseCurrents, BaseWaves, BaseWind, Circle, CircleList, CurrentsOverride,
    EventId, GlwEvent, GlwEventKey, GustsPercent, KnotSpeed, MarginMeters, Overlap,
    PercentVariance, Period, RadiusMeters, ShiftsDegrees, WaterDepth, WaterDepthSetting,
    WaveEffects, WaveHeight, WaveLength, WavesOverride, WindDirection, WindOverride,
};
