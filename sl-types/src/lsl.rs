//! LSL types and parsers/printers to use LSL format for them

#[cfg(feature = "chumsky")]
use chumsky::{
    Parser,
    prelude::{Simple, just},
    text::whitespace,
};

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
pub fn vector_parser() -> impl Parser<char, Vector, Error = Simple<char>> {
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
pub fn rotation_parser() -> impl Parser<char, Rotation, Error = Simple<char>> {
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
