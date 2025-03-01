//! Types related to SL nearby avatar radar and enter/leave messages

#[cfg(feature = "chumsky")]
use chumsky::{
    prelude::{just, Simple},
    Parser,
};

/// represents a Second Life area of significance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumIs)]
pub enum Area {
    /// chat range
    ChatRange,
    /// draw distance
    DrawDistance,
    /// region
    Region,
}

/// parse a SecondLifeArea
///
/// # Errors
///
/// returns and error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn area_parser() -> impl Parser<char, Area, Error = Simple<char>> {
    just("chat range")
        .to(Area::ChatRange)
        .or(just("draw distance").to(Area::DrawDistance))
        .or(just("region").to(Area::Region))
}
