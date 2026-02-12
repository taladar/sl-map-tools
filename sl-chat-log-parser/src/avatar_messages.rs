//! Avatar related messages (those sent by an avatar as well as some system messages about an avatar like coming online or entering chat range)

use chumsky::IterParser as _;
use chumsky::Parser;
use chumsky::prelude::{any, choice, just};

/// represents a Second Life avatar related message
#[derive(Debug, Clone, PartialEq)]
pub enum AvatarMessage {
    /// a message about the avatar whispering, saying or shouting something
    Chat {
        /// how "loud" the message was (whisper, say, shout or region say)
        volume: sl_types::chat::ChatVolume,
        /// the chat message
        message: String,
    },
    /// an emote (chat message starting with /me in the log)
    Emote {
        /// how "loud" the message was (whisper, say, shout or region say)
        volume: sl_types::chat::ChatVolume,
        /// the chat message without the /me
        message: String,
    },
    /// a message about an avatar coming online
    CameOnline,
    /// a message about an avatar going offline
    WentOffline,
    /// a message about an avatar entering an area of significance
    EnteredArea {
        /// the area of significance
        area: sl_types::radar::Area,
        /// the distance where the avatar entered the area
        distance: Option<sl_types::map::Distance>,
    },
    /// a message about an avatar leaving an area of significance
    LeftArea {
        /// the area of significance
        area: sl_types::radar::Area,
    },
}

/// parse a Second Life avatar chat message
///
/// # Errors
///
/// returns an error if the parser fails
fn avatar_chat_message_parser<'src>()
-> impl Parser<'src, &'src str, AvatarMessage, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    any()
        .repeated()
        .collect::<String>()
        .try_map(|s, _span: chumsky::span::SimpleSpan| {
            let (v, s) = sl_types::chat::ChatVolume::volume_and_message(s.to_string());
            Ok(AvatarMessage::Chat {
                volume: v,
                message: s,
            })
        })
}

/// parse a Second Life avatar emote message
///
/// # Errors
///
/// returns an error if the parser fails
fn avatar_emote_message_parser<'src>()
-> impl Parser<'src, &'src str, AvatarMessage, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("/me ")
        .ignore_then(any().repeated().collect::<String>())
        .try_map(|s, _span: chumsky::span::SimpleSpan| {
            let (v, s) = sl_types::chat::ChatVolume::volume_and_message(s);
            Ok(AvatarMessage::Emote {
                volume: v,
                message: s,
            })
        })
}

/// parse a message about an avatar coming online
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
pub fn avatar_came_online_message_parser<'src>()
-> impl Parser<'src, &'src str, AvatarMessage, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("is online.").map(|_| AvatarMessage::CameOnline)
}

/// parse a message about an avatar going offline
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
pub fn avatar_went_offline_message_parser<'src>()
-> impl Parser<'src, &'src str, AvatarMessage, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("is offline.").map(|_| AvatarMessage::WentOffline)
}

/// parse a message about an avatar entering an area of significance
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
pub fn avatar_entered_area_message_parser<'src>()
-> impl Parser<'src, &'src str, AvatarMessage, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("entered ")
        .ignore_then(sl_types::radar::area_parser())
        .then(
            just(" (")
                .ignore_then(sl_types::map::distance_parser())
                .then_ignore(just(")"))
                .or_not(),
        )
        .then_ignore(just("."))
        .try_map(|(area, distance), _span: chumsky::span::SimpleSpan| {
            Ok(AvatarMessage::EnteredArea { area, distance })
        })
}

/// parse a message about an avatar leaving an area of significance
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
pub fn avatar_left_area_message_parser<'src>()
-> impl Parser<'src, &'src str, AvatarMessage, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("left ")
        .ignore_then(sl_types::radar::area_parser())
        .then_ignore(just("."))
        .try_map(|area, _span: chumsky::span::SimpleSpan| Ok(AvatarMessage::LeftArea { area }))
}

/// parse a Second Life avatar message
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
pub fn avatar_message_parser<'src>()
-> impl Parser<'src, &'src str, AvatarMessage, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    choice([
        avatar_came_online_message_parser().boxed(),
        avatar_went_offline_message_parser().boxed(),
        avatar_entered_area_message_parser().boxed(),
        avatar_left_area_message_parser().boxed(),
        avatar_emote_message_parser().boxed(),
        avatar_chat_message_parser().boxed(),
    ])
}
