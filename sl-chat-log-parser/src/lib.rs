#![doc = include_str!("../README.md")]

use chumsky::error::Simple;
use chumsky::prelude::{any, just, none_of, one_of, take_until};
use chumsky::text::whitespace;
use chumsky::Parser;

pub mod avatar_messages;
pub mod system_messages;
pub mod utils;

/// represents an event commemorated in the Second Life chat log
#[derive(Debug, Clone, PartialEq)]
pub enum ChatLogEvent {
    /// line about an avatar (or an object doing things indistinguishable from an avatar in the chat log)
    AvatarLine {
        /// name of the avatar or object
        name: String,
        /// message
        message: Box<crate::avatar_messages::AvatarMessage>,
    },
    /// a message by the Second Life viewer or server itself
    SystemMessage {
        /// the system message
        message: Box<crate::system_messages::SystemMessage>,
    },
    /// a message without a colon, most likely an unnamed object like a translator, spanker, etc.
    OtherMessage {
        /// the message
        message: String,
    },
}

/// parse a second life avatar name as it appears in the chat log before a message
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
pub fn avatar_name_parser() -> impl Parser<char, String, Error = Simple<char>> {
    none_of(":")
        .repeated()
        .collect::<String>()
        .try_map(|s, _span: std::ops::Range<usize>| Ok(s))
}

/// parse a Second Life chat log event
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
fn chat_log_event_parser() -> impl Parser<char, ChatLogEvent, Error = Simple<char>> {
    just("Second Life: ")
        .ignore_then(
            take_until(
                crate::avatar_messages::avatar_came_online_message_parser().or(
                    crate::avatar_messages::avatar_went_offline_message_parser()
                        .or(crate::avatar_messages::avatar_entered_area_message_parser())
                        .or(crate::avatar_messages::avatar_left_area_message_parser()),
                ),
            )
            .map(|(vc, msg)| (vc.into_iter().collect::<String>(), msg))
            .map(|(name, message)| ChatLogEvent::AvatarLine {
                name: name.strip_suffix(" ").unwrap_or(&name).to_owned(),
                message: Box::new(message),
            }),
        )
        .or(
            just("Second Life: ").ignore_then(crate::system_messages::system_message_parser().map(
                |message| ChatLogEvent::SystemMessage {
                    message: Box::new(message),
                },
            )),
        )
        .or(avatar_name_parser()
            .then_ignore(just(":").then(whitespace()))
            .then(crate::avatar_messages::avatar_message_parser())
            .map(|(name, message)| ChatLogEvent::AvatarLine {
                name,
                message: Box::new(message),
            }))
        .or(any()
            .repeated()
            .collect::<String>()
            .map(|s| ChatLogEvent::OtherMessage { message: s }))
}

/// represents a Second Life chat log line
#[derive(Debug, Clone, PartialEq)]
pub struct ChatLogLine {
    /// timestamp of the chat log line, some log lines do not have one because of bugs at the time they were written (e.g. some just have the time formatting string)
    pub timestamp: Option<time::PrimitiveDateTime>,
    /// event that happened at that time
    pub event: ChatLogEvent,
}

/// parse a Second Life chat log line
///
/// # Errors
///
/// returns an error if the parser fails
#[must_use]
pub fn chat_log_line_parser() -> impl Parser<char, ChatLogLine, Error = Simple<char>> {
    just("[")
        .ignore_then(
            one_of("0123456789")
                .repeated()
                .exactly(4)
                .collect::<String>(),
        )
        .then(
            just("/").ignore_then(
                one_of("0123456789")
                    .repeated()
                    .exactly(2)
                    .collect::<String>(),
            ),
        )
        .then(
            just("/").ignore_then(
                one_of("0123456789")
                    .repeated()
                    .exactly(2)
                    .collect::<String>(),
            ),
        )
        .then(
            just(" ").ignore_then(
                one_of("0123456789")
                    .repeated()
                    .exactly(2)
                    .collect::<String>(),
            ),
        )
        .then(
            just(":").ignore_then(
                one_of("0123456789")
                    .repeated()
                    .exactly(2)
                    .collect::<String>(),
            ),
        )
        .then(
            just(":")
                .ignore_then(
                    one_of("0123456789")
                        .repeated()
                        .exactly(2)
                        .collect::<String>(),
                )
                .or_not(),
        )
        .then_ignore(just("]"))
        .try_map(
            |(((((year, month), day), hour), minute), second),
             span: std::ops::Range<usize>| {
                let second = second.unwrap_or("00".to_string());
                let format = time::macros::format_description!(
                    "[year]/[month]/[day] [hour]:[minute]:[second]"
                );
                Ok(Some(
                    time::PrimitiveDateTime::parse(
                        &format!("{}/{}/{} {}:{}:{}", year, month, day, hour, minute, second),
                        format,
                    ).map_err(|e| Simple::custom(span, format!("{:?}", e)))?
                ))
             }
        )
        .or(just("[[year,datetime,slt]/[mthnum,datetime,slt]/[day,datetime,slt] [hour,datetime,slt]:[min,datetime,slt]]").map(|_| None))
        .then_ignore(whitespace())
        .then(chat_log_event_parser())
        .try_map(
            |(timestamp, event),
             _span: std::ops::Range<usize>| {
                Ok(ChatLogLine {
                    timestamp,
                    event,
                })
            },
        )
}

#[cfg(test)]
mod test {
    use std::io::{BufRead, BufReader};

    use super::*;

    /// used to deserialize the required options from the environment
    #[derive(Debug, serde::Deserialize)]
    struct EnvOptions {
        #[serde(
            deserialize_with = "serde_aux::field_attributes::deserialize_vec_from_string_or_vec"
        )]
        test_avatar_names: Vec<String>,
    }

    /// Error enum for the application
    #[derive(thiserror::Error, Debug)]
    pub enum TestError {
        /// error loading environment
        #[error("error loading environment: {0}")]
        EnvError(#[from] envy::Error),
        /// error loading .env file
        #[error("error loading .env file: {0}")]
        DotEnvError(#[from] dotenvy::Error),
        /// error determining current user home directory
        #[error("error determining current user home directory")]
        HomeDirError,
        /// error opening chat log file
        #[error("error opening chat log file {0}: {1}")]
        OpenChatLogFileError(std::path::PathBuf, std::io::Error),
        /// error reading chat log line from file
        #[error("error reading chat log line from file: {0}")]
        ChatLogLineReadError(std::io::Error),
    }

    /// determine avatar log dir from avatar name
    pub fn avatar_log_dir(avatar_name: &str) -> Result<std::path::PathBuf, TestError> {
        let avatar_dir_name = avatar_name.replace(' ', "_").to_lowercase();
        tracing::debug!("Avatar dir name: {}", avatar_dir_name);

        let Some(home_dir) = dirs2::home_dir() else {
            tracing::error!("Could not determine current user home directory");
            return Err(TestError::HomeDirError);
        };

        Ok(home_dir.join(".firestorm/").join(avatar_dir_name))
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_log_line_parser() -> Result<(), TestError> {
        dotenvy::dotenv()?;
        let env_options = envy::from_env::<EnvOptions>()?;
        for avatar_name in env_options.test_avatar_names {
            let avatar_dir = avatar_log_dir(&avatar_name)?;
            let local_chat_log_file = avatar_dir.join("chat.txt");
            let file = std::fs::File::open(&local_chat_log_file)
                .map_err(|e| TestError::OpenChatLogFileError(local_chat_log_file.clone(), e))?;
            let file = BufReader::new(file);
            let mut last_line: Option<String> = None;
            for line in file.lines() {
                let line = line.map_err(TestError::ChatLogLineReadError)?;
                if line.starts_with(" ") || line == "" {
                    if let Some(ll) = last_line {
                        last_line = Some(format!("{}\n{}", ll, line));
                        continue;
                    }
                }
                if let Some(ref ll) = last_line {
                    match chat_log_line_parser().parse(ll.clone()) {
                        Err(e) => {
                            tracing::error!("failed to parse line\n{}", ll);
                            for err in e {
                                tracing::error!("{}", err);
                            }
                            panic!("Failed to parse a line");
                        }
                        Ok(parsed_line) => {
                            if let ChatLogLine {
                                timestamp: _,
                                event:
                                    ChatLogEvent::SystemMessage {
                                        message:
                                            system_messages::SystemMessage::OtherSystemMessage {
                                                ref message,
                                            },
                                    },
                            } = parsed_line
                            {
                                tracing::info!("parsed line\n{}\n{:?}", ll, parsed_line);
                                if message.starts_with("The message sent to") {
                                    if let Err(e) =
                                        system_messages::chat_message_still_being_processed_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!("{}", utils::ChumskyError {
                                                description: "group chat message still being processed".to_string(),
                                                source: message.to_owned(),
                                                errors: vec![e.to_owned()],
                                            });
                                        }
                                    }
                                }
                                if message.contains("owned by") && message.contains("gave you") {
                                    if let Err(e) =
                                        system_messages::object_gave_object_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "owned by gave you".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.contains("An object named")
                                    && message.contains("gave you this folder")
                                {
                                    if let Err(e) =
                                        system_messages::object_gave_folder_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description:
                                                        "An object named ... gave you this folder"
                                                            .to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.starts_with("Can't rez object")
                                    && message.contains(
                                        "because the owner of this land does not allow it",
                                    )
                                {
                                    if let Err(e) =
                                        system_messages::permission_to_rez_object_denied_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!("{}", utils::ChumskyError {
                                                description: "permission to rez object denied".to_string(),
                                                source: message.to_owned(),
                                                errors: vec![e.to_owned()],
                                            });
                                        }
                                    }
                                }
                                if message.starts_with("Teleport completed from") {
                                    if let Err(e) =
                                        system_messages::teleport_completed_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "teleported completed".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.starts_with("[")
                                    && message.contains("status.secondlifegrid.net")
                                {
                                    if let Err(e) =
                                        system_messages::grid_status_event_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "grid status event".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.starts_with("Object ID:") {
                                    if let Err(e) =
                                        system_messages::extended_script_info_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "extended script info".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.starts_with("Bridge") {
                                    if let Err(e) = system_messages::bridge_message_parser()
                                        .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "bridge message".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }

                                if message.starts_with("You paid") {
                                    if let Err(e) = system_messages::sent_payment_message_parser()
                                        .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "sent payment".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.contains("Take Linden dollars") {
                                    if let Err(e) = system_messages::object_granted_permission_to_take_money_parser()
                                        .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "object granted permission to take money".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.starts_with("You have offered a calling card") {
                                    if let Err(e) =
                                        system_messages::offered_calling_card_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "offered calling card".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.starts_with("Draw Distance set") {
                                    if let Err(e) =
                                        system_messages::draw_distance_set_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "draw distance set".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                                if message.starts_with("Your object") {
                                    if let Err(e) =
                                        system_messages::your_object_has_been_returned_message_parser()
                                            .parse(message.to_string())
                                    {
                                        for e in e {
                                            tracing::debug!(
                                                "{}",
                                                utils::ChumskyError {
                                                    description: "your object has been returned".to_string(),
                                                    source: message.to_owned(),
                                                    errors: vec![e.to_owned()],
                                                }
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                last_line = Some(line);
            }
        }
        // enable to see output during development, both to identity unhandled messages and to see parse errors above
        //panic!();
        Ok(())
    }
}
