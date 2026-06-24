//! LSL types and parsers/printers to use LSL format for them

#[cfg(feature = "chumsky")]
use chumsky::{Parser, prelude::just, text::whitespace};

#[cfg(feature = "chumsky")]
use crate::utils::f32_parser;

/// LSL Vector of 3 float components
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    /// x component
    pub x: f32,
    /// y component
    pub y: f32,
    /// z component
    pub z: f32,
}

/// parse an LSL vector
///
/// "<1.234,3.456,4.567>"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn vector_parser<'src>()
-> impl Parser<'src, &'src str, Vector, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just('<')
        .then(whitespace().or_not())
        .ignore_then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just(','))
        .then_ignore(whitespace().or_not())
        .then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just(','))
        .then_ignore(whitespace().or_not())
        .then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just('>'))
        .map(|((x, y), z)| Vector { x, y, z })
}

impl From<crate::map::RegionCoordinates> for Vector {
    fn from(value: crate::map::RegionCoordinates) -> Self {
        Self {
            x: value.x(),
            y: value.y(),
            z: value.z(),
        }
    }
}

/// LSL Rotation (quaternion) of 4 float components
#[derive(Debug, Clone, PartialEq)]
pub struct Rotation {
    /// x component
    pub x: f32,
    /// y component
    pub y: f32,
    /// z component
    pub z: f32,
    /// s component
    pub s: f32,
}

/// parse an LSL rotation
///
/// "<1.234,3.456,4.567,5.678>"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn rotation_parser<'src>()
-> impl Parser<'src, &'src str, Rotation, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just('<')
        .then(whitespace().or_not())
        .ignore_then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just(','))
        .then_ignore(whitespace().or_not())
        .then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just(','))
        .then_ignore(whitespace().or_not())
        .then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just(','))
        .then_ignore(whitespace().or_not())
        .then(f32_parser())
        .then_ignore(whitespace().or_not())
        .then_ignore(just('>'))
        .map(|(((x, y), z), s)| Rotation { x, y, z, s })
}

/// The permissions an in-world script may request via `llRequestPermissions`, a
/// bitfield shared by the `ScriptQuestion` (request) and `ScriptAnswerYes`
/// (grant) messages. The flag values match the LSL `PERMISSION_*` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptPermissions(pub i32);

impl ScriptPermissions {
    /// Debit the agent's account (`PERMISSION_DEBIT`).
    pub const DEBIT: i32 = 1 << 1;
    /// Take control inputs (`PERMISSION_TAKE_CONTROLS`).
    pub const TAKE_CONTROLS: i32 = 1 << 2;
    /// Trigger animations on the agent (`PERMISSION_TRIGGER_ANIMATION`).
    pub const TRIGGER_ANIMATION: i32 = 1 << 4;
    /// Attach to the agent (`PERMISSION_ATTACH`).
    pub const ATTACH: i32 = 1 << 5;
    /// Change link-set membership (`PERMISSION_CHANGE_LINKS`).
    pub const CHANGE_LINKS: i32 = 1 << 7;
    /// Track the agent's camera (`PERMISSION_TRACK_CAMERA`).
    pub const TRACK_CAMERA: i32 = 1 << 10;
    /// Control the agent's camera (`PERMISSION_CONTROL_CAMERA`).
    pub const CONTROL_CAMERA: i32 = 1 << 11;
    /// Teleport the agent (`PERMISSION_TELEPORT`).
    pub const TELEPORT: i32 = 1 << 12;
    /// Participate in an experience (`PERMISSION_EXPERIENCE`).
    pub const EXPERIENCE: i32 = 1 << 13;
    /// Silently manage estate access (`PERMISSION_SILENT_ESTATE_MANAGEMENT`).
    pub const SILENT_ESTATE_MANAGEMENT: i32 = 1 << 14;
    /// Override the agent's animations (`PERMISSION_OVERRIDE_ANIMATIONS`).
    pub const OVERRIDE_ANIMATIONS: i32 = 1 << 15;
    /// Return objects (`PERMISSION_RETURN_OBJECTS`).
    pub const RETURN_OBJECTS: i32 = 1 << 16;

    /// Whether all of the bits in `mask` are granted/requested.
    #[must_use]
    pub const fn contains(self, mask: i32) -> bool {
        self.0 & mask == mask
    }
}
