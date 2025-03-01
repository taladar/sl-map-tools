//! Types and parsers for system messages in the chat log

use chumsky::error::Simple;
use chumsky::prelude::{any, just, take_until};
use chumsky::text::whitespace;
use chumsky::Parser;

/// represents a Second Life system message
#[derive(Debug, Clone, PartialEq)]
pub enum SystemMessage {
    /// message about a saved snapshot
    SavedSnapshotMessage {
        /// the snapshot filename
        filename: std::path::PathBuf,
    },
    /// message about a saved attachment
    AttachmentSavedMessage,
    /// message about a sent payment
    SentPaymentMessage {
        /// the recipient avatar UUID
        recipient_avatar_key: sl_types::key::AgentKey,
        /// the amount paid
        amount: sl_types::money::LindenAmount,
        /// when buying an object the name of the object
        object_name: Option<String>,
    },
    /// message about a received payment
    ReceivedPaymentMessage {
        /// the sender avatar UUID
        sender_avatar_key: sl_types::key::AgentKey,
        /// the amount received
        amount: sl_types::money::LindenAmount,
        /// an optional message
        message: Option<String>,
    },
    /// message about a song playing on stream
    NowPlayingMessage {
        /// the song name
        song_name: String,
    },
    /// message about a completed teleport
    TeleportCompletedMessage {
        /// teleported originated at this location
        origin: sl_types::map::Location,
    },
    /// message about a region restart of the region that the avatar is in
    RegionRestartMessage,
    /// message about an object giving the current avatar an object
    ObjectGaveObjectMessage {
        /// the giving object name
        giving_object_name: String,
        /// the giving object location
        giving_object_location: sl_types::map::Location,
        /// the giving object owner
        giving_object_owner: sl_types::key::AgentKey,
        /// the name of the given object
        given_object_name: String,
    },
    /// message about an avatar giving the current avatar an object
    AvatarGaveObjectMessage {
        /// the giving avatar name
        giving_avatar_name: String,
        /// the name of the given object
        given_object_name: String,
    },
    /// message about successfully shared items
    ItemsSuccessfullyShared,
    /// message about a modified search query
    ModifiedSearchQuery {
        /// the modified query
        query: String,
    },
    /// message about different simulator version
    SimulatorVersion {
        /// the previous region simulator version
        previous_region_simulator_version: String,
        /// the current region simulator version
        current_region_simulator_version: String,
    },
    /// message about a renamed avatar
    RenamedAvatar {
        /// the old name
        old_name: String,
        /// the new name
        new_name: String,
    },
    /// other system message
    OtherSystemMessage {
        /// the raw message
        message: String,
    },
}

/// parse a system message about a saved snapshot
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn snapshot_saved_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Snapshot saved: ")
        .ignore_then(
            any()
                .repeated()
                .collect::<String>()
                .map(std::path::PathBuf::from),
        )
        .try_map(|filename, _span: std::ops::Range<usize>| {
            Ok(SystemMessage::SavedSnapshotMessage { filename })
        })
}

/// parse a system message about a saved attachment
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn attachment_saved_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Attachment has been saved")
        .try_map(|_, _span: std::ops::Range<usize>| Ok(SystemMessage::AttachmentSavedMessage))
}

/// parse a system message about a sent payment
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn sent_payment_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You paid ")
        .ignore_then(sl_types::key::app_agent_uri_as_agent_key_parser())
        .then_ignore(just(" "))
        .then(sl_types::money::linden_amount_parser())
        .then(
            just(" for ")
                .ignore_then(take_until(just(".")).map(|(n, _)| Some(n)))
                .or(just(".").map(|_| None)),
        )
        .try_map(
            |((recipient_avatar_key, amount), object_name), _span: std::ops::Range<usize>| {
                Ok(SystemMessage::SentPaymentMessage {
                    recipient_avatar_key,
                    amount,
                    object_name: object_name.map(|n| n.into_iter().collect()),
                })
            },
        )
}

/// parse a system message about a received payment
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn received_payment_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    sl_types::key::app_agent_uri_as_agent_key_parser()
        .then_ignore(just(" paid you "))
        .then(sl_types::money::linden_amount_parser())
        .then(
            just(": ")
                .ignore_then(any().repeated().collect::<String>())
                .ignore_then(take_until(just(".")).map(|(n, _)| Some(n)))
                .or(just(".").map(|_| None)),
        )
        .try_map(
            |((sender_avatar_key, amount), message), _span: std::ops::Range<usize>| {
                Ok(SystemMessage::ReceivedPaymentMessage {
                    sender_avatar_key,
                    amount,
                    message: message.map(|n| n.into_iter().collect()),
                })
            },
        )
}

/// parse a system message about a completed teleport
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn teleport_completed_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("Teleport completed from http://maps.secondlife.com/secondlife/")
        .ignore_then(sl_types::map::location_parser())
        .try_map(|origin, _span: std::ops::Range<usize>| {
            Ok(SystemMessage::TeleportCompletedMessage { origin })
        })
}

/// parse a system message about a now playing song
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn now_playing_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Now playing: ")
        .ignore_then(any().repeated().collect::<String>())
        .try_map(|song_name, _span: std::ops::Range<usize>| {
            Ok(SystemMessage::NowPlayingMessage { song_name })
        })
}

/// parse a system message about a region restart
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn region_restart_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("The region you are in now is about to restart. If you stay in this region you will be logged out.")
        .try_map(|_, _span: std::ops::Range<usize>| {
            Ok(SystemMessage::RegionRestartMessage)
        })
}

/// parse a system message about an object giving the current avatar an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn object_gave_object_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    take_until(just(" owned by "))
        .then(sl_types::key::app_agent_uri_as_agent_key_parser())
        .then_ignore(
            whitespace()
                .or_not()
                .ignore_then(just("gave you ").then(just("<nolink>'").or_not())),
        )
        .then(take_until(
            just("</nolink>'")
                .or_not()
                .then(whitespace())
                .then(just("( http://slurl.com/secondlife/")),
        ))
        .then(sl_types::map::location_parser())
        .then_ignore(just(" )."))
        .try_map(
            |(
                (((giving_object_name, _), giving_object_owner), (given_object_name, _)),
                giving_object_location,
            ),
             _span: std::ops::Range<usize>| {
                Ok(SystemMessage::ObjectGaveObjectMessage {
                    giving_object_name: giving_object_name.into_iter().collect(),
                    giving_object_owner,
                    given_object_name: given_object_name.into_iter().collect(),
                    giving_object_location,
                })
            },
        )
}

/// parse a system message about an avatar giving the current avatar an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn avatar_gave_object_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("A group member named ")
        .ignore_then(take_until(just(" gave you ")))
        .then(take_until(just(".")))
        .try_map(
            |((giving_avatar_name, _), (given_object_name, _)), _span: std::ops::Range<usize>| {
                Ok(SystemMessage::AvatarGaveObjectMessage {
                    giving_avatar_name: giving_avatar_name.into_iter().collect(),
                    given_object_name: given_object_name.into_iter().collect(),
                })
            },
        )
}

/// parse a system message about items being successfully shared
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn items_successfully_shared_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Items successfully shared.")
        .try_map(|_, _span: std::ops::Range<usize>| Ok(SystemMessage::ItemsSuccessfullyShared))
}

/// parse a system message about a modified search query
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn modified_search_query_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Your search query was modified and the words that were too short were removed.")
        .ignore_then(whitespace())
        .ignore_then(just("Searched for:"))
        .ignore_then(whitespace())
        .ignore_then(any().repeated().collect::<String>())
        .try_map(|query, _span: std::ops::Range<usize>| {
            Ok(SystemMessage::ModifiedSearchQuery { query })
        })
}

/// parse a system message about a different simulator version
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn simulator_version_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("The region you have entered is running a different simulator version.")
        .ignore_then(whitespace())
        .ignore_then(just("Current simulator:"))
        .ignore_then(whitespace())
        .ignore_then(take_until(just("\n")).map(|(s, _): (Vec<char>, _)| s.into_iter().collect()))
        .then_ignore(whitespace())
        .then_ignore(just("Previous simulator:"))
        .then_ignore(whitespace())
        .then(any().repeated().collect::<String>())
        .try_map(
            |(current_region_simulator_version, previous_region_simulator_version),
             _span: std::ops::Range<usize>| {
                Ok(SystemMessage::SimulatorVersion {
                    previous_region_simulator_version,
                    current_region_simulator_version,
                })
            },
        )
}

/// parse a system message about a renamed avatar
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn renamed_avatar_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    take_until(just(" is now known as"))
        .map(|(s, _)| s.into_iter().collect())
        .then_ignore(whitespace())
        .then(take_until(just(".")).map(|(s, _): (Vec<char>, _)| s.into_iter().collect()))
        .try_map(|(old_name, new_name), _span: std::ops::Range<usize>| {
            Ok(SystemMessage::RenamedAvatar { old_name, new_name })
        })
}

/// parse a Second Life system message
///
/// TODO:
/// You decline...
/// Creating bridge...
/// Bridge created...
/// Script info...
/// Unable to initiate teleport due to RLV restrictions
/// Gave you messages without nolink tags
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn system_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    snapshot_saved_message_parser().or(attachment_saved_message_parser().or(
        sent_payment_message_parser().or(received_payment_message_parser().or(
            teleport_completed_message_parser().or(now_playing_message_parser().or(
                region_restart_message_parser().or(object_gave_object_message_parser().or(
                    items_successfully_shared_message_parser().or(
                        modified_search_query_message_parser().or(
                            avatar_gave_object_message_parser().or(
                                simulator_version_message_parser().or(
                                    renamed_avatar_message_parser().or(any()
                                        .repeated()
                                        .collect::<String>()
                                        .try_map(|s, _span: std::ops::Range<usize>| {
                                            Ok(SystemMessage::OtherSystemMessage { message: s })
                                        })),
                                ),
                            ),
                        ),
                    ),
                )),
            )),
        )),
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_teleport_completed() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            Ok(SystemMessage::TeleportCompletedMessage { origin: sl_types::map::Location { region_name: sl_types::map::RegionName::try_new("Girlywood")?, x: 30, y: 169, z: 912 } }),
            teleport_completed_message_parser().parse("Teleport completed from http://maps.secondlife.com/secondlife/Girlywood/30/169/912")
        );
        Ok(())
    }
}
