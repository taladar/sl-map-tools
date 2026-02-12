//! Pathfinding related types

#[cfg(feature = "chumsky")]
use chumsky::Parser;

/// Pathfinding types
///
/// see <https://wiki.secondlife.com/wiki/Category:LSL_Pathfinding_Types>
#[derive(Debug, Clone, Hash, PartialEq, Eq, strum::FromRepr, strum::EnumIs)]
#[repr(i8)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is used outside this module"
)]
pub enum PathfindingType {
    /// Attachments, Linden trees & grass
    Other = -1,
    /// Movable obstacles, movable phantoms, physical, and volumedetect objects
    LegacyLinkset = 0,
    /// Avatars
    Avatar = 1,
    /// Pathfinding characters
    Character = 2,
    /// Walkable objects
    Walkable = 3,
    /// Static obstacles
    StaticObstacle = 4,
    /// Material volumes
    MaterialVolume = 5,
    /// Exclusion volumes
    ExclusionVolume = 6,
}

/// parse a signed integer as a pathfinding type based on the C/LSL constant
/// values
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn int_as_pathfinding_type_parser<'src>()
-> impl Parser<'src, &'src str, PathfindingType, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    crate::utils::i8_parser().try_map(|repr, span| {
        crate::pathfinding::PathfindingType::from_repr(repr).ok_or_else(|| {
            chumsky::error::Rich::custom(
                span,
                "Could not convert parsed pathfinding type i8 into PathfindingType enum",
            )
        })
    })
}
