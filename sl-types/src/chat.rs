//! Types related to SL chat

/// represents a Second Life chat channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChatChannel(pub i32);

impl std::fmt::Display for ChatChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// the public chat channel on Second Life
pub const PUBLIC_CHANNEL: ChatChannel = ChatChannel(0);

/// the combat log event chat channel on Second Life
pub const COMBAT_CHANNEL: ChatChannel = ChatChannel(0x7FFFFFFE);

/// the script debug chat channel on Second Life
pub const DEBUG_CHANNEL: ChatChannel = ChatChannel(0x7FFFFFFF);

/// represents a Second Life chat volume
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumIs)]
pub enum ChatVolume {
    /// whisper (10m)
    Whisper,
    /// say (20m, default, a.k.a. chat range)
    Say,
    /// shout (100m)
    Shout,
    /// region say (the whole region)
    RegionSay,
}
