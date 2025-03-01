//! Types related to SL nearby avatar radar and enter/leave messages

/// represents a Second Life area of significance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumIs)]
pub enum SecondLifeArea {
    /// chat range
    ChatRange,
    /// draw distance
    DrawDistance,
    /// region
    Region,
}
