//! Types and parsers for system messages in the chat log

use chumsky::error::Simple;
use chumsky::prelude::{any, choice, just, none_of, one_of, take_until};
use chumsky::text::{digits, newline, whitespace};
use chumsky::Parser;
use sl_types::utils::{i64_parser, u64_parser, unsigned_f32_parser, usize_parser};

/// represents a Second Life system message
#[derive(Debug, Clone, PartialEq)]
pub enum SystemMessage {
    /// message about a saved snapshot
    SavedSnapshotMessage {
        /// the snapshot filename
        filename: std::path::PathBuf,
    },
    /// message about a failure to save a snapshot due to missing destination folder
    FailedToSaveSnapshotDueToMissingDestinationFolder {
        /// the snapshot folder
        folder: std::path::PathBuf,
    },
    /// message about a failure to save a snapshot due to disk space
    FailedToSaveSnapshotDueToDiskSpace {
        /// the snapshot folder
        folder: std::path::PathBuf,
        /// the amount of space required
        required_disk_space: bytesize::ByteSize,
        /// the amount of free space reported
        free_disk_space: bytesize::ByteSize,
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
    /// message about paying to join a group
    YouPaidToJoinGroupMessage {
        /// the group key for the joined group
        joined_group: sl_types::key::GroupKey,
        /// the amount paid to join
        join_fee: sl_types::money::LindenAmount,
    },
    /// message that you have been added to a group
    AddedToGroup,
    /// message that you left a group
    LeftGroup {
        /// the name of the group left
        group_name: String,
    },
    /// message that you are unable to invite a user to a group because you
    /// are not in the group
    UnableToInviteUserDueToMissingGroupMembership,
    /// message that loading a notecard failed
    UnableToLoadNotecard,
    /// message about a song playing on stream
    NowPlayingMessage {
        /// the song name
        song_name: String,
    },
    /// message about a completed teleport
    TeleportCompletedMessage {
        /// teleported originated at this location
        origin: sl_types::map::UnconstrainedLocation,
    },
    /// message about a region restart of the region that the avatar is in
    RegionRestartMessage,
    /// message about an object giving the current avatar an object
    ObjectGaveObjectMessage {
        /// the giving object name
        giving_object_name: String,
        /// the giving object location
        giving_object_location: sl_types::map::UnconstrainedLocation,
        /// the giving object owner
        giving_object_owner: sl_types::key::AgentKey,
        /// the name of the given object
        given_object_name: String,
    },
    /// message about an avatar giving the current avatar an object
    AvatarGaveObjectMessage {
        /// is the giving avatar a group member
        is_group_member: bool,
        /// the giving avatar name
        giving_avatar_name: String,
        /// the name of the given object
        given_object_name: String,
    },
    /// message about you declining an object given to you
    DeclinedGivenObject {
        /// the name of the declined object
        object_name: String,
        /// the location of the giver
        giver_location: sl_types::map::UnconstrainedLocation,
        /// the name of the giver
        giver_name: String,
    },
    /// message asking to select residents to share with
    SelectResidentsToShareWith,
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
    /// message about enabling or disabling double-click teleports
    DoubleClickTeleport {
        /// whether this event enables or disables double-click teleports
        enabled: bool,
    },
    /// message that the bridge creation started
    CreatingBridge,
    /// message that the bridge was created
    BridgeCreated,
    /// message that the bridge creation is still in progress and another one
    /// can not be created simultaneously
    BridgeCreationInProgress,
    /// message that the bridge failed to attach
    BridgeFailedToAttach,
    /// message that the bridge was not created
    BridgeNotCreated,
    /// message that the bridge was detached
    BridgeDetached,
    /// script count changed
    ScriptCountChanged {
        /// script count before
        previous_script_count: u32,
        /// script count now
        current_script_count: u32,
        /// change
        change: i32,
    },
    /// the group chat message is still being processed
    GroupChatMessageStillBeingProcessed {
        /// the name of the group
        group_name: String,
    },
    /// the object is not for sale
    ObjectNotForSale,
    /// link failed because pieces being too far apart
    LinkFailedDueToPieceDistance {
        /// link failed for this many pieces
        link_failed_pieces: usize,
        /// total selected pieces
        total_selected_pieces: usize,
    },
    /// rezzing an object failed because the parcel is full
    RezObjectFailedDueToFullParcel {
        /// name of the object
        object_name: String,
        /// name of the parcel
        parcel_name: String,
        /// attempted rez location
        attempted_rez_location: sl_types::map::RegionCoordinates,
        /// name of the region where the rez failed
        region_name: sl_types::map::RegionName,
    },
    /// permission to rez an object denied
    PermissionToRezObjectDenied {
        /// name of the object
        object_name: String,
        /// name of the parcel
        parcel_name: String,
        /// attempted rez location
        attempted_rez_location: sl_types::map::RegionCoordinates,
        /// name of the region where the rez failed
        region_name: sl_types::map::RegionName,
    },
    /// permission to reposition an object denied
    PermissionToRepositionDenied,
    /// permission to rotate an object denied
    PermissionToRotateDenied,
    /// permission to rescale an object denied
    PermissionToRescaleDenied,
    /// permission to unlink denied due to missing build permissions on at least one parcel
    PermissionToUnlinkDeniedDueToMissingParcelBuildPermissions,
    /// permission to view script denied
    PermissionToViewScriptDenied,
    /// permission to view notecard denied
    PermissionToViewNotecardDenied,
    /// permission to enter parcel denied
    PermissionToEnterParcelDenied,
    /// permission to enter parcel denied due to ban
    PermissionToEnterParcelDeniedDueToBan,
    /// ejected from parcel
    EjectedFromParcel,
    /// no longer allowed and ejected
    EjectedFromParcelBecauseNoLongerAllowed,
    /// banned temporarily
    BannedFromParcelTemporarily {
        /// How long the ban lasts
        ban_duration: time::Duration,
    },
    /// banned indefinitely
    BannedFromParcelIndefinitely,
    /// only group members can visit this area
    OnlyGroupMembersCanVisitThisArea,
    /// unable to teleport due to RLV restriction
    UnableToTeleportDueToRlv,
    /// unable to open texture due to RLV restriction
    UnableToOpenTextureDueToRlv,
    /// unsupported SLurl
    UnsupportedSlurl,
    /// SLurl from untrusted browser blocked
    BlockedUntrustedBrowserSlurl,
    /// grid status error invalid message format
    GridStatusErrorInvalidMessageFormat,
    /// script info object is invalid or out of range
    ScriptInfoObjectInvalidOrOutOfRange,
    /// script info
    ScriptInfo {
        /// name of the object or avatar whose script info this is
        name: String,
        /// running scripts
        running_scripts: usize,
        /// total scripts
        total_scripts: usize,
        /// allowed memory size limit
        allowed_memory_size_limit: bytesize::ByteSize,
        /// CPU time consumed
        cpu_time_consumed: time::Duration,
    },
    /// Firestorm extended script info
    ExtendedScriptInfo {
        /// object key
        object_key: sl_types::key::ObjectKey,
        /// description of the inspected object
        description: Option<String>,
        /// key of the room prim
        root_prim: sl_types::key::ObjectKey,
        /// prim count
        prim_count: usize,
        /// land impact
        land_impact: usize,
        /// number of items in the inspect object's inventory
        inventory_items: usize,
        /// velocity
        velocity: sl_types::lsl::Vector,
        /// position in the region
        position: sl_types::map::RegionCoordinates,
        /// distance from inspecting avatar to position of inspected object
        position_distance: sl_types::map::Distance,
        /// rotation of the inspected object as a quaternion
        rotation: sl_types::lsl::Rotation,
        /// rotation of the inspected object as a vector of angles in degrees
        rotation_vector_degrees: sl_types::lsl::Vector,
        /// angular velocity of the inspected object in radians per second
        angular_velocity: sl_types::lsl::Vector,
        /// creator
        creator: sl_types::key::AgentKey,
        /// owner
        owner: sl_types::key::OwnerKey,
        /// previous owner
        previous_owner: Option<sl_types::key::OwnerKey>,
        /// rezzed by
        rezzed_by: sl_types::key::AgentKey,
        /// group
        group: Option<sl_types::key::GroupKey>,
        /// creation time
        creation_time: Option<time::OffsetDateTime>,
        /// rez time
        rez_time: Option<time::OffsetDateTime>,
        /// pathfinding type
        pathfinding_type: sl_types::pathfinding::PathfindingType,
        /// attachment point
        attachment_point: Option<sl_types::attachment::AttachmentPoint>,
        /// temporarily attached
        temporarily_attached: bool,
        /// inspecting avatar position
        inspecting_avatar_position: sl_types::map::RegionCoordinates,
    },
    /// a message from the Firestorm developers
    FirestormMessage {
        /// the type of message, basically whatever follows the initial
        /// Firestorm up until the exclamation mark (e.g. Tip, Help, Classes,...)
        message_type: String,
        /// the actual message, everything after the exclamation mark
        message: String,
    },
    /// message about a grid status event
    GridStatusEvent {
        /// event title
        title: String,
        /// is this a scheduled event
        scheduled: bool,
        /// event body
        body: String,
        /// event URL
        incident_url: String,
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
        .map(|filename| SystemMessage::SavedSnapshotMessage { filename })
        .or(just("Failed to save snapshot to ").ignore_then(
            take_until(just(": Directory does not exist.").ignored()).map(|(folder, _)| {
                SystemMessage::FailedToSaveSnapshotDueToMissingDestinationFolder {
                    folder: std::path::PathBuf::from(folder.into_iter().collect::<String>()),
                }
            }),
        ))
        .or(just("Failed to save snapshot to ").ignore_then(
            take_until(just(": Disk is full. ").ignored())
                .map(|(folder, _)| std::path::PathBuf::from(folder.into_iter().collect::<String>()))
                .then(u64_parser())
                .then_ignore(just("KB is required but only "))
                .then(u64_parser())
                .then_ignore(just("KB is free."))
                .map(|((folder, required), free)| {
                    let required_disk_space = bytesize::ByteSize::kib(required);
                    let free_disk_space = bytesize::ByteSize::kib(free);
                    SystemMessage::FailedToSaveSnapshotDueToDiskSpace {
                        folder,
                        required_disk_space,
                        free_disk_space,
                    }
                }),
        ))
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

/// parse a system message about paying to join a group
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn you_paid_to_join_group_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You paid ")
        .ignore_then(sl_types::viewer_uri::viewer_app_group_uri_parser())
        .then_ignore(whitespace())
        .then(sl_types::money::linden_amount_parser())
        .then_ignore(just(" to join a group."))
        .try_map(|(group_uri, join_fee), span| match group_uri {
            sl_types::viewer_uri::ViewerUri::GroupAbout(group_key)
            | sl_types::viewer_uri::ViewerUri::GroupInspect(group_key) => {
                Ok(SystemMessage::YouPaidToJoinGroupMessage {
                    joined_group: group_key,
                    join_fee,
                })
            }
            other => Err(Simple::custom(
                span,
                format!(
                    "Unexpected type of group URI in group join message: {:?}",
                    other
                ),
            )),
        })
}

/// parse a system message about being added or leaving a group
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn group_membership_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You have been added to the group.")
        .to(SystemMessage::AddedToGroup)
        .or(just("You have left the group '")
            .ignore_then(none_of('\'').repeated().collect::<String>())
            .then_ignore(just("'."))
            .map(|group_name| SystemMessage::LeftGroup { group_name }))
}

/// parse a system message about the inability to invite a user to a group
/// you yourself are not a member of
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_invite_user_due_to_missing_group_membership_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to invite user because you are not in that group.")
        .to(SystemMessage::UnableToInviteUserDueToMissingGroupMembership)
}

/// parse a system message about the inability to load a notecard
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_load_notecard_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to load the notecard.")
        .then_ignore(newline())
        .then_ignore(whitespace())
        .then(just("Please try again."))
        .to(SystemMessage::UnableToLoadNotecard)
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
        .ignore_then(sl_types::map::unconstrained_location_parser())
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
        .then(sl_types::map::unconstrained_location_parser())
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
        .or_not()
        .then(take_until(just(" gave you ")))
        .then(take_until(just(".")))
        .try_map(
            |((group_member, (giving_avatar_name, _)), (given_object_name, _)),
             _span: std::ops::Range<usize>| {
                Ok(SystemMessage::AvatarGaveObjectMessage {
                    is_group_member: group_member.is_some(),
                    giving_avatar_name: giving_avatar_name.into_iter().collect(),
                    given_object_name: given_object_name.into_iter().collect(),
                })
            },
        )
}

/// parse a system message about declining an object given to you
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn declined_given_object_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You decline '")
        .ignore_then(
            take_until(just("'  ( http://slurl.com/secondlife/").ignored())
                .map(|(vc, _)| vc.into_iter().collect::<String>()),
        )
        .then(sl_types::map::unconstrained_location_parser())
        .then_ignore(just(" ) from "))
        .then(
            any()
                .repeated()
                .collect::<String>()
                .map(|s| s.strip_suffix(".").map(|s| s.to_string()).unwrap_or(s)),
        )
        .map(
            |((object_name, giver_location), giver_name)| SystemMessage::DeclinedGivenObject {
                object_name,
                giver_location,
                giver_name,
            },
        )
}

/// You decline '<object name>' ( http://slurl.com/secondlife/<location> ) from <giving object name>.

/// parse a system message asking to select residents to share with
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn select_residents_to_share_with_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Select residents to share with.").to(SystemMessage::SelectResidentsToShareWith)
}

/// parse a system message about items being successfully shared
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn items_successfully_shared_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Items successfully shared.").to(SystemMessage::ItemsSuccessfullyShared)
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

/// parse a system message about enabling or disabling of double-click teleports
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn doubleclick_teleport_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("DoubleClick Teleport enabled.")
        .to(SystemMessage::DoubleClickTeleport { enabled: true })
        .or(just("DoubleClick Teleport disabled.")
            .to(SystemMessage::DoubleClickTeleport { enabled: false }))
}

/// parse a system message about the LSL viewer bridge
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn bridge_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Creating the bridge. This might take a moment, please wait.").to(SystemMessage::CreatingBridge)
    .or(just("Bridge created.").to(SystemMessage::BridgeCreated))
    .or(just("Bridge creation in process, cannot start another. Please wait a few minutes before trying again.").to(SystemMessage::BridgeCreationInProgress))
    .or(just("Bridge failed to attach. This is not the current bridge version. Please use the Firestorm 'Avatar/Avatar Health/Recreate Bridge' menu option to recreate the bridge.").to(SystemMessage::BridgeFailedToAttach))
    .or(just("Bridge not created. The bridge couldn't be found in inventory. Please use the Firestorm 'Avatar/Avatar Health/Recreate Bridge' menu option to recreate the bridge.").to(SystemMessage::BridgeNotCreated))
    .or(just("Bridge detached.").to(SystemMessage::BridgeDetached))
}

/// parse a system message about a changed script count in the current region
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn region_script_count_change_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Total scripts in region ")
        .ignore_then(just("jumped from ").or(just("dropped from ")))
        .ignore_then(
            digits(10)
                .then_ignore(just(" to "))
                .then(digits(10))
                .then_ignore(just(" ("))
                .then(one_of("+-"))
                .then(digits(10))
                .then_ignore(just(")."))
                .try_map(
                    |(((previous_script_count, current_script_count), sign), diff): (
                        ((String, String), char),
                        String,
                    ),
                     span: std::ops::Range<usize>| {
                        let previous_span = span.clone();
                        let previous_script_count =
                            previous_script_count.parse().map_err(|err| {
                                Simple::custom(
                                    previous_span,
                                    format!(
                                        "Could not parse previous script count ({}) as u32: {:?}",
                                        previous_script_count, err
                                    ),
                                )
                            })?;
                        let current_span = span.clone();
                        let current_script_count = current_script_count.parse().map_err(|err| {
                            Simple::custom(
                                current_span,
                                format!(
                                    "Could not parse current script count ({}) as u32: {:?}",
                                    current_script_count, err
                                ),
                            )
                        })?;
                        let diff_span = span.clone();
                        let diff: i32 = diff.parse().map_err(|err| {
                            Simple::custom(
                                diff_span,
                                format!(
                                    "Could not parse changed script count ({}) as i32: {:?}",
                                    diff, err
                                ),
                            )
                        })?;
                        let change = match sign {
                            '+' => diff,
                            '-' => -diff,
                            c => {
                                return Err(Simple::custom(
                                    span,
                                    format!("Unexpected sign character for script change: {}", c),
                                ))
                            }
                        };
                        Ok(SystemMessage::ScriptCountChanged {
                            previous_script_count,
                            current_script_count,
                            change,
                        })
                    },
                ),
        )
}

/// parse a system message about a group chat message still being processed
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn group_chat_message_still_being_processed_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("The message sent to ")
        .ignore_then(take_until(just(" is still being processed.").ignored()).map(|(vc, _)| vc.into_iter().collect::<String>()))
        .then_ignore(newline())
        .then_ignore(whitespace())
        .then_ignore(just("If the message does not appear in the next few minutes, it may have been dropped by the server."))
        .map(|group_name| {
            SystemMessage::GroupChatMessageStillBeingProcessed {
                group_name,
            }
        })
}

/// parse a system message about an object not being for sale
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn object_not_for_sale_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("This object is not for sale.").to(SystemMessage::ObjectNotForSale)
}

/// parse a system message about a failed link due to piece distance
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn link_failed_due_to_piece_distance_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Link failed -- Unable to link ").ignore_then(
        usize_parser()
            .then_ignore(just(" of the "))
            .then(usize_parser())
            .then_ignore(just(" selected pieces - pieces are too far apart."))
            .map(|(link_failed_pieces, total_selected_pieces)| {
                SystemMessage::LinkFailedDueToPieceDistance {
                    link_failed_pieces,
                    total_selected_pieces,
                }
            }),
    )
}

/// parse a system message about the failure to rez an object due to a full
/// parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn rezzing_object_failed_due_to_full_parcel_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rez object '").ignore_then(
        take_until(just("' at ").ignored())
            .map(|(vc, _)| vc.into_iter().collect::<String>())
            .then(sl_types::map::region_coordinates_parser())
            .then_ignore(just(" on parcel '"))
            .then(
                take_until(just("' in region ").ignored())
                    .map(|(vc, _)| vc.into_iter().collect::<String>()),
            )
            .then(
                take_until(just(" because the parcel is too full").ignored())
                    .map(|(vc, _)| vc.into_iter().collect::<String>())
                    .try_map(|region_name, span| {
                        sl_types::map::RegionName::try_new(&region_name).map_err(|err| {
                            Simple::custom(
                                span,
                                format!(
                                    "Could not turn parsed region name ({}) into RegionName: {:?}",
                                    region_name, err
                                ),
                            )
                        })
                    }),
            )
            .map(
                |(((object_name, attempted_rez_location), parcel_name), region_name)| {
                    SystemMessage::RezObjectFailedDueToFullParcel {
                        object_name,
                        attempted_rez_location,
                        parcel_name,
                        region_name,
                    }
                },
            ),
    )
}

/// parse a system message about the denial of permission to rez an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_rez_object_denied_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rez object '")
        .ignore_then(
            take_until(just("' at ").ignored()).map(|(vc, _)| vc.into_iter().collect::<String>())
            .then(sl_types::map::region_coordinates_parser())
            .then_ignore(just(" on parcel '"))
            .then(take_until(just("' in region ").ignored()).map(|(vc, _)| vc.into_iter().collect::<String>()))
            .then(take_until(just(" because the owner of this land does not allow it.  Use the land tool to see land ownership.").ignored()).map(|(vc, _)| vc.into_iter().collect::<String>()).try_map(|region_name, span| {
                sl_types::map::RegionName::try_new(&region_name).map_err(|err| Simple::custom(span, format!("Could not turn parsed region name ({}) into RegionName: {:?}", region_name, err)))
            }))
            .map(|(((object_name, attempted_rez_location), parcel_name), region_name)| {
                SystemMessage::PermissionToRezObjectDenied {
                    object_name,
                    attempted_rez_location,
                    parcel_name,
                    region_name,
                }
            })
        )
}

/// parse a system message about the denial of permission to reposition an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_reposition_denied_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't reposition -- permission denied").to(SystemMessage::PermissionToRepositionDenied)
}

/// parse a system message about the denial of permission to rotate an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_rotate_denied_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rotate -- permission denied").to(SystemMessage::PermissionToRotateDenied)
}

/// parse a system message about the denial of permission to rescale an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_rescale_denied_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rescale -- permission denied").to(SystemMessage::PermissionToRescaleDenied)
}

/// parse a system message about the denial of permission to unlink an object
/// because build permissions are missing on at least one parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_unlink_denied_due_to_missing_parcel_build_permissions_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Failed to unlink because you do not have permissions to build on all parcels")
        .to(SystemMessage::PermissionToUnlinkDeniedDueToMissingParcelBuildPermissions)
}

/// parse a system message about the denial of permission to view a script
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_view_script_denied_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Insufficient permissions to view the script.")
        .to(SystemMessage::PermissionToViewScriptDenied)
}

/// parse a system message about the denial of permission to view a notecard
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_view_notecard_denied_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You do not have permission to view this notecard.")
        .to(SystemMessage::PermissionToViewNotecardDenied)
}

/// parse a system message about the denial of permission to enter a parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_enter_parcel_denied_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Cannot enter parcel, you are not on the access list.")
        .to(SystemMessage::PermissionToEnterParcelDenied)
}

/// parse a system message about the denial of permission to enter a parcel due to ban
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_enter_parcel_denied_due_to_ban_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Cannot enter parcel, you have been banned.")
        .to(SystemMessage::PermissionToEnterParcelDeniedDueToBan)
}

/// parse a system message about being ejected from a parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn ejected_from_parcel_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("You have been ejected from this land.")
        .to(SystemMessage::EjectedFromParcel)
        .or(
            just("You are no longer allowed here and have been ejected.")
                .to(SystemMessage::EjectedFromParcelBecauseNoLongerAllowed),
        )
}

/// parse a system message about being banned from a parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn banned_from_parcel_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("You have been banned ").ignore_then(
        just("indefinitely")
            .to(SystemMessage::BannedFromParcelIndefinitely)
            .or(just("for ")
                .ignore_then(i64_parser().then_ignore(just(" minutes")))
                .map(|d| SystemMessage::BannedFromParcelTemporarily {
                    ban_duration: time::Duration::minutes(d),
                })),
    )
}

/// parse a system message about only group members being able to visit an area
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn only_group_members_can_visit_this_area_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Only members of a certain group can visit this area.")
        .to(SystemMessage::OnlyGroupMembersCanVisitThisArea)
}

/// parse a system message about teleports being RLV restricted
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_teleport_due_to_rlv_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to initiate teleport due to RLV restrictions")
        .to(SystemMessage::UnableToTeleportDueToRlv)
}

/// parse a system message about opening textures being RLV restricted
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_open_texture_due_to_rlv_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to open texture due to RLV restrictions")
        .to(SystemMessage::UnableToOpenTextureDueToRlv)
}

/// parse a system message about unsupported SLurl
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unsupported_slurl_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("The SLurl you clicked on is not supported.").to(SystemMessage::UnsupportedSlurl)
}

/// parse a system message about a SLurl from an untrusted browser being blocked
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn blocked_untrusted_browser_slurl_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("A SLurl was received from an untrusted browser and has been blocked for your security")
        .to(SystemMessage::BlockedUntrustedBrowserSlurl)
}

/// parse a system message about a grid status error about an invalid message format
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn grid_status_error_invalid_message_format_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("SL Grid Status error: Invalid message format. Try again later.")
        .to(SystemMessage::GridStatusErrorInvalidMessageFormat)
}

/// parse a system message about a script info object being invalid or out of range
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn script_info_object_invalid_or_out_of_range_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Script info: Object to check is invalid or out of range.")
        .to(SystemMessage::ScriptInfoObjectInvalidOrOutOfRange)
}

/// parse a system message about script info
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn script_info_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Script info: '").ignore_then(
        take_until(just("': [").ignored())
            .map(|(vc, _)| vc.into_iter().collect::<String>())
            .then(usize_parser())
            .then_ignore(just('/'))
            .then(usize_parser())
            .then_ignore(just("] running scripts, "))
            .then(u64_parser().map(bytesize::ByteSize::kb))
            .then_ignore(just(" KB allowed memory size limit, "))
            .then(unsigned_f32_parser().map(|ms| time::Duration::seconds_f32(ms / 1000f32)))
            .then_ignore(just(" ms of CPU time consumed."))
            .map(
                |(
                    (((name, running_scripts), total_scripts), allowed_memory_size_limit),
                    cpu_time_consumed,
                )| {
                    SystemMessage::ScriptInfo {
                        name,
                        running_scripts,
                        total_scripts,
                        allowed_memory_size_limit,
                        cpu_time_consumed,
                    }
                },
            ),
    )
}

/// parse a system message with extended script info
/// usually this should follow a line with regular script info containing the
/// object name
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn extended_script_info_message_parser(
) -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Object ID: ")
        .ignore_then(sl_types::key::object_key_parser())
        .then_ignore(newline())
        .then_ignore(just(" Description:"))
        .then_ignore(just(" ").or_not())
        .then(just("(No Description)").then_ignore(newline()).to(None).or(
            take_until(newline().ignored()).map(|(vc, _)| Some(vc.into_iter().collect::<String>())),
        ))
        .then_ignore(just(" Root prim: "))
        .then(sl_types::key::object_key_parser())
        .then_ignore(newline())
        .then_ignore(just(" Prim count: "))
        .then(sl_types::utils::usize_parser())
        .then_ignore(newline())
        .then_ignore(just(" Land impact: "))
        .then(sl_types::utils::usize_parser())
        .then_ignore(newline())
        .then_ignore(just(" Inventory items: "))
        .then(sl_types::utils::usize_parser())
        .then_ignore(newline())
        .then_ignore(just(" Velocity: "))
        .then(sl_types::lsl::vector_parser())
        .then_ignore(newline())
        .then_ignore(just(" Position: "))
        .then(sl_types::lsl::vector_parser().map(sl_types::map::RegionCoordinates::from))
        .then_ignore(whitespace())
        .then(sl_types::map::distance_parser().delimited_by(just('('), just(')')))
        .then_ignore(newline())
        .then_ignore(just(" Rotation: "))
        .then(sl_types::lsl::rotation_parser())
        .then_ignore(whitespace())
        .then(sl_types::lsl::vector_parser().delimited_by(just('('), just(')')))
        .then_ignore(newline())
        .then_ignore(just(" Angular velocity: "))
        .then(sl_types::lsl::vector_parser())
        .then_ignore(whitespace())
        .then_ignore(just("(radians per second)"))
        .then_ignore(newline())
        .then_ignore(just(" Creator: "))
        .then(sl_types::key::app_agent_uri_as_agent_key_parser())
        .then_ignore(newline())
        .then_ignore(just(" Owner: "))
        .then(sl_types::key::app_agent_or_group_uri_as_owner_key_parser())
        .then_ignore(newline())
        .then_ignore(just(" Previous owner: "))
        .then(
            sl_types::key::app_agent_or_group_uri_as_owner_key_parser()
                .map(Some)
                .or(just("---").to(None)),
        )
        .then_ignore(newline())
        .then_ignore(just(" Rezzed by: "))
        .then(sl_types::key::agent_key_parser())
        .then_ignore(newline())
        .then_ignore(just(" Group: "))
        .then(
            sl_types::key::app_group_uri_as_group_key_parser()
                .map(Some)
                .or(just("---").to(None)),
        )
        .then_ignore(newline())
        .then_ignore(just(" Creation time:"))
        .then_ignore(just(' ').or_not())
        .then(crate::utils::offset_datetime_parser().or_not())
        .then_ignore(newline())
        .then_ignore(just(" Rez time:"))
        .then_ignore(just(' ').or_not())
        .then(crate::utils::offset_datetime_parser().or_not())
        .then_ignore(newline())
        .then_ignore(just(" Pathfinding type: "))
        .then(sl_types::pathfinding::int_as_pathfinding_type_parser())
        .then_ignore(newline())
        .then_ignore(just(" Attachment point: "))
        .then(
            sl_types::attachment::attachment_point_parser()
                .map(Some)
                .or(just("---").to(None)),
        )
        .then_ignore(newline())
        .then_ignore(just(" Temporarily attached: "))
        .then(just("Yes").to(true).or(just("No").to(false)))
        .then_ignore(newline())
        .then_ignore(just(" Your current position: "))
        .then(sl_types::lsl::vector_parser().map(sl_types::map::RegionCoordinates::from))
        .map(
            |((((((((((((((((((((((
                object_key,
                description),
                root_prim),
                prim_count),
                land_impact),
                inventory_items),
                velocity),
                position),
                position_distance),
                rotation),
                rotation_vector_degrees),
                angular_velocity),
                creator),
                owner),
                previous_owner),
                rezzed_by),
                group),
                creation_time),
                rez_time),
                pathfinding_type),
                attachment_point),
                temporarily_attached),
                inspecting_avatar_position,
            )| {
                SystemMessage::ExtendedScriptInfo {
                    object_key,
                    description,
                    root_prim,
                    prim_count,
                    land_impact,
                    inventory_items,
                    velocity,
                    position,
                    position_distance,
                    rotation,
                    rotation_vector_degrees,
                    angular_velocity,
                    creator,
                    owner,
                    previous_owner,
                    rezzed_by,
                    group,
                    creation_time,
                    rez_time,
                    pathfinding_type,
                    attachment_point,
                    temporarily_attached,
                    inspecting_avatar_position,
                }
            },
        )
}

/// Script info: 'icon': [3/3] running scripts, 192 KB allowed memory size limit, 0.012550 ms of CPU time consumed.

/// parse a system message by the Firestorm developers
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn firestorm_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Firestorm ").ignore_then(
        take_until(just("!").ignored())
            .map(|(message_type, _)| message_type.into_iter().collect::<String>())
            .then(any().repeated().collect::<String>())
            .map(|(message_type, message)| SystemMessage::FirestormMessage {
                message_type,
                message,
            }),
    )
}

/// parse a system message about a grid status event
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn grid_status_event_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("[ ").ignore_then(
        take_until(just(" ] "))
            .map(|(vc, _)| vc.into_iter().collect::<String>())
            .then(
                just("THIS IS A SCHEDULED EVENT ")
                    .or_not()
                    .map(|s| s.is_some()),
            )
            .then(
                take_until(just(" [ https://status.secondlifegrid.net/incidents/").ignored())
                    .map(|(vc, _)| vc.into_iter().collect::<String>()),
            )
            .then(take_until(just(' ').ignored()).map(|(vc, _)| vc.into_iter().collect::<String>()))
            .then_ignore(just("]"))
            .map(
                |(((title, scheduled), body), url_fragment)| SystemMessage::GridStatusEvent {
                    title,
                    scheduled,
                    body,
                    incident_url: format!(
                        "https://status.secondlifegird.net/incidents/{}",
                        url_fragment
                    ),
                },
            ),
    )
}

/// parse a Second Life system message
///
/// TODO:
/// ... gave you ... (no location URL, quotes,...)
/// Gave you messages (without nolink tags)
/// You have offered a calling card to <avatar name>.
/// You paid secondlife:///app/agent/<agent key>/inspect L$<amount>: <avatar name>
/// secondlife:///app/agent/<agent key>/inspect paid you L$<amount>: <your avatar name>
/// '<object name>', an object owned by '<avatar name>', located in (unknown region) at (unknown position), has been granted permission to: Take Linden dollars (L$) from you.
/// '<object name>', an object owned by '<avatar name>', located in <region name> at <raw position without any delimiters and apparently missing a space after the comma between y and z>, has been granted permission to: Take Linden dollars (L$) from you.
/// The message sent to Multi-person chat is still being processed.\n If the message does not appear in the next few minutes, it may have been dropped by the server.
/// The message sent to (IM Session Doesn't Exist) is still being processed.\n If the message does not appear in the next few minutes, it may have been dropped by the server.
/// The message sent to Conference with <avatar name> is still being processed.
/// An object named [secondlife:///app/objectim/00000000-0000-0000-0000-000000000000/?name=Gift%20from%20Mithlumen&owner=99338959-f536-4719-b91b-21a8bd72a1b0&slurl=The%20Seventh%20Valley%2F129%2F116%2F2500 Gift from Mithlumen] gave you this folder: 'Gift from Mithlumen'
/// Draw Distance set to <distance>m.
/// You have been banned for 178 minutes
/// Audio from the domain <domain (not url, just a bare domain)> will always be played.
/// You cannot create objects here.  The owner of this land does not allow it.  Use the land tool to see land ownership.
/// Always Run enabled.
/// You are not allowed to change this shape.
/// You must provide positive values for dice (max 100) and faces (max 1000).
/// #5 1d6: 3.
/// Total result for 5d6: 15.
/// Avatar ejected.
/// You failed to pay secondlife:///app/agent/<agent key>/inspect L$<amount>.
/// Texture info for: <object name>
/// 512x512 opaque on face 0
/// 1024x1024 alpha on face 0
/// You paid secondlife:///app/group/<group key>/inspect L$<amount> for a parcel of land.
/// You have left the group '<group name (with ')>'.
/// Land has been divided.
/// Home position set.
/// Cannot create requested inventory.
/// Failed to place object at specified location.  Please try again.
/// Bridge object not found. Can't proceed with creation, exiting.
/// Bridge failed to attach. Something else was using the bridge attachment point. Please try to recreate the bridge.
/// Creating the bridge. This might take a few moments, please wait
/// Unable to create requested object. The region is full.
/// Initializing World...
/// Initializing multimedia...
/// Loading fonts...
/// Decoding images...
/// Waiting for region handshake...
/// Welcome to Advertisement-Free Firestorm
/// Connecting to region...
/// Loading world...
/// Downloading clothing...
/// Logging in...
/// Logging in. Firestorm may appear frozen.  Please wait.
/// This is a test version of Firestorm. If this were an actual release version, a real message of the day would be here. This is only a test.
/// You have been added as an estate manager.
/// Your object 'Bandit IF B' has been returned to your inventory Lost and Found folder from parcel 'Gulf of Moles' at Dague 146, 144 due to parcel auto return.
/// <avatar name> has declined your call.  You will now be reconnected to Nearby Voice Chat.
///
/// broken timestamps: [[year,datetime,slt]/[mthnum,datetime,slt]/[day,datetime,slt] [hour,datetime,slt]:[min,datetime,slt]] (already handled, seems to not be working everywhere?)
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn system_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    choice([
        snapshot_saved_message_parser().boxed(),
        attachment_saved_message_parser().boxed(),
        sent_payment_message_parser().boxed(),
        received_payment_message_parser().boxed(),
        you_paid_to_join_group_message_parser().boxed(),
        group_membership_message_parser().boxed(),
        unable_to_invite_user_due_to_missing_group_membership_message_parser().boxed(),
        unable_to_load_notecard_message_parser().boxed(),
        teleport_completed_message_parser().boxed(),
        now_playing_message_parser().boxed(),
        region_restart_message_parser().boxed(),
        object_gave_object_message_parser().boxed(),
        declined_given_object_message_parser().boxed(),
        select_residents_to_share_with_message_parser().boxed(),
        items_successfully_shared_message_parser().boxed(),
        modified_search_query_message_parser().boxed(),
        avatar_gave_object_message_parser().boxed(),
        simulator_version_message_parser().boxed(),
        renamed_avatar_message_parser().boxed(),
        doubleclick_teleport_message_parser().boxed(),
        bridge_message_parser().boxed(),
        region_script_count_change_message_parser().boxed(),
        group_chat_message_still_being_processed_message_parser().boxed(),
        object_not_for_sale_message_parser().boxed(),
        link_failed_due_to_piece_distance_message_parser().boxed(),
        rezzing_object_failed_due_to_full_parcel_message_parser().boxed(),
        permission_to_rez_object_denied_message_parser().boxed(),
        permission_to_reposition_denied_message_parser().boxed(),
        permission_to_rotate_denied_message_parser().boxed(),
        permission_to_rescale_denied_message_parser().boxed(),
        permission_to_unlink_denied_due_to_missing_parcel_build_permissions_message_parser()
            .boxed(),
        permission_to_view_script_denied_message_parser().boxed(),
        permission_to_view_notecard_denied_message_parser().boxed(),
        permission_to_enter_parcel_denied_message_parser().boxed(),
        permission_to_enter_parcel_denied_due_to_ban_message_parser().boxed(),
        ejected_from_parcel_message_parser().boxed(),
        banned_from_parcel_message_parser().boxed(),
        only_group_members_can_visit_this_area_message_parser().boxed(),
        unable_to_teleport_due_to_rlv_message_parser().boxed(),
        unable_to_open_texture_due_to_rlv_message_parser().boxed(),
        unsupported_slurl_message_parser().boxed(),
        blocked_untrusted_browser_slurl_message_parser().boxed(),
        grid_status_error_invalid_message_format_message_parser().boxed(),
        script_info_object_invalid_or_out_of_range_message_parser().boxed(),
        script_info_message_parser().boxed(),
        extended_script_info_message_parser().boxed(),
        firestorm_message_parser().boxed(),
        grid_status_event_message_parser().boxed(),
        any()
            .repeated()
            .collect::<String>()
            .try_map(|s, _span: std::ops::Range<usize>| {
                Ok(SystemMessage::OtherSystemMessage { message: s })
            })
            .boxed(),
    ])
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_teleport_completed() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            Ok(SystemMessage::TeleportCompletedMessage {
                origin: sl_types::map::UnconstrainedLocation {
                    region_name: sl_types::map::RegionName::try_new("Fudo")?,
                    x: 30,
                    y: 169,
                    z: 912
                }
            }),
            teleport_completed_message_parser().parse(
                "Teleport completed from http://maps.secondlife.com/secondlife/Fudo/30/169/912"
            )
        );
        Ok(())
    }

    #[test]
    fn test_teleport_completed_extra_short() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            Ok(SystemMessage::TeleportCompletedMessage {
                origin: sl_types::map::UnconstrainedLocation {
                    region_name: sl_types::map::RegionName::try_new("AA")?,
                    x: 78,
                    y: 83,
                    z: 26
                }
            }),
            teleport_completed_message_parser()
                .parse("Teleport completed from http://maps.secondlife.com/secondlife/AA/78/83/26")
        );
        Ok(())
    }

    #[test]
    fn test_cant_rez_object() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            Ok(SystemMessage::PermissionToRezObjectDenied {
                object_name: "Foo2".to_string(),
                attempted_rez_location: sl_types::map::RegionCoordinates::new(63.0486, 45.2515, 1501.08),
                parcel_name: "The Foo Bar".to_string(),
                region_name: sl_types::map::RegionName::try_new("Fudo")?,
            }),
            permission_to_rez_object_denied_message_parser()
                .parse("Can't rez object 'Foo2' at { 63.0486, 45.2515, 1501.08 } on parcel 'The Foo Bar' in region Fudo because the owner of this land does not allow it.  Use the land tool to see land ownership.")
        );
        Ok(())
    }
}
