//! Types related to SL chat

#[cfg(feature = "chumsky")]
use chumsky::{prelude::Simple, text::digits, Parser};

/// represents a Second Life chat channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChatChannel(pub i32);

/// parse a chat channel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn chat_channel_parser() -> impl Parser<char, ChatChannel, Error = Simple<char>> {
    digits(10).try_map(|d: <char as chumsky::text::Character>::Collection, span| {
        let d: i32 = d
            .parse()
            .map_err(|e| Simple::custom(span, format!("{:?}", e)))?;
        Ok(ChatChannel(d))
    })
}

impl std::fmt::Display for ChatChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ChatChannel {
    type Err = <i32 as std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <i32 as std::str::FromStr>::from_str(s).map(ChatChannel)
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

impl ChatVolume {
    /// identify the chat volume of a message and strip it off the message
    #[must_use]
    pub fn volume_and_message(s: String) -> (ChatVolume, String) {
        if let Some(whisper_message) = s.strip_prefix("whispers: ") {
            (ChatVolume::Whisper, whisper_message.to_string())
        } else if let Some(shout_message) = s.strip_prefix("shouts: ") {
            (ChatVolume::Shout, shout_message.to_string())
        } else {
            (ChatVolume::Say, s)
        }
    }
}
