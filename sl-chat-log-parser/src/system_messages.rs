//! Types and parsers for system messages in the chat log

use chumsky::Parser;
use chumsky::error::Simple;
use chumsky::prelude::{any, choice, end, just, one_of, take_until};
use chumsky::text::{digits, newline, whitespace};
use sl_types::utils::{i64_parser, u64_parser, unsigned_f32_parser, usize_parser};

/// represents a Second Life system message
#[derive(Debug, Clone, PartialEq)]
pub enum SystemMessage {
    /// message about a saved snapshot
    SavedSnapshot {
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
    /// message about the draw distance being set to a specific value
    DrawDistanceSet {
        /// the distance the draw distance was set to
        distance: sl_types::map::Distance,
    },
    /// message about the home position being set
    HomePositionSet,
    /// message about land being divided
    LandDivided,
    /// message about a failure to join land due to region boundary
    FailedToJoinLandDueToRegionBoundary,
    /// message about offering a calling card
    OfferedCallingCard {
        /// the name of the avatar we offered a calling card to
        recipient_avatar_name: String,
    },
    /// message about a saved attachment
    AttachmentSavedMessage,
    /// message about paying for an object
    YouPaidForObject {
        /// the seller avatar or group
        seller: sl_types::key::OwnerKey,
        /// the amount paid
        amount: sl_types::money::LindenAmount,
        /// the name of the object you paid for
        object_name: String,
    },
    /// message about paying to create a group
    YouPaidToCreateGroup {
        /// the agent you paid
        payment_recipient: sl_types::key::AgentKey,
        /// the amount you paid
        amount: sl_types::money::LindenAmount,
    },
    /// message about paying to join a group
    YouPaidToJoinGroup {
        /// the group key for the joined group
        joined_group: sl_types::key::GroupKey,
        /// the amount paid to join
        join_fee: sl_types::money::LindenAmount,
    },
    /// message about paying for a parcel of land
    YouPaidForLand {
        /// previous land owner
        previous_land_owner: sl_types::key::OwnerKey,
        /// the amount paid
        amount: sl_types::money::LindenAmount,
    },
    /// message about a failed payment
    FailedToPay {
        /// payment recipient
        payment_recipient: sl_types::key::OwnerKey,
        /// the amount that could not be paid
        amount: sl_types::money::LindenAmount,
    },
    /// message about an object being granted permission to take L$
    ObjectGrantedPermissionToTakeMoney {
        /// the name of the object
        object_name: String,
        /// the owner of the object
        owner_name: String,
        /// the region where the object is located
        object_region: Option<sl_types::map::RegionName>,
        /// the coordinates within that region
        object_location: Option<sl_types::map::RegionCoordinates>,
    },
    /// message about a sent payment
    SentPayment {
        /// the recipient avatar or group key
        recipient_key: sl_types::key::OwnerKey,
        /// the amount sent
        amount: sl_types::money::LindenAmount,
        /// an optional message
        message: Option<String>,
    },
    /// message about a received payment
    ReceivedPayment {
        /// the sender avatar or group key
        sender_key: sl_types::key::OwnerKey,
        /// the amount received
        amount: sl_types::money::LindenAmount,
        /// an optional message
        message: Option<String>,
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
    /// message that you are unable to invite a user to a group because the
    /// user is in a different limited estate than the group
    UnableToInviteUserToGroupDueToDifferingLimitedEstate,
    /// message that loading a notecard failed
    UnableToLoadNotecard,
    /// message that loading a gesture failed
    UnableToLoadGesture {
        /// name of the gesture that could not be loaded
        gesture_name: String,
    },
    /// message about a song playing on stream
    NowPlaying {
        /// the song name
        song_name: String,
    },
    /// message about a completed teleport
    TeleportCompleted {
        /// teleported originated at this location
        origin: sl_types::map::UnconstrainedLocation,
    },
    /// message about a region restart of the region that the avatar is in
    RegionRestart,
    /// message about an object giving the current avatar an object
    ObjectGaveObject {
        /// the giving object name
        giving_object_name: String,
        /// the giving object location
        giving_object_location: sl_types::map::UnconstrainedLocation,
        /// the giving object owner
        giving_object_owner: sl_types::key::OwnerKey,
        /// the name of the given object
        given_object_name: String,
    },
    /// message about an object giving the current avatar a folder
    ObjectGaveFolder {
        /// key of the object
        giving_object_key: sl_types::key::ObjectKey,
        /// name of the object
        giving_object_name: String,
        /// owner of the object
        giving_object_owner: sl_types::key::OwnerKey,
        /// object location
        giving_object_location: sl_types::map::Location,
        /// giving object link label
        giving_object_link_label: String,
        /// given folder name
        folder_name: String,
    },
    /// message about an avatar giving the current avatar an object
    AvatarGaveObject {
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
    /// message about enabling or disabling always run
    AlwaysRun {
        /// whether this event enables or disables always run
        enabled: bool,
    },
    /// message about being added as an estate manager
    AddedAsEstateManager,
    /// message that the bridge creation started
    CreatingBridge,
    /// message that the bridge was created
    BridgeCreated,
    /// message that the bridge creation is still in progress and another one
    /// can not be created simultaneously
    BridgeCreationInProgress,
    /// message that the bridge failed to attach
    BridgeFailedToAttach,
    /// message that the bridge failed to attach because something else is using
    /// the bridge attachment point
    BridgeFailedToAttachDueToBridgeAttachmentPointInUse,
    /// message that the bridge was not created
    BridgeNotCreated,
    /// message that the bridge was detached
    BridgeDetached,
    /// message that the bridge object was not found and the creation was aborted
    BridgeObjectNotFoundCantProceedWithCreation,
    /// failed to place object at specified location, please try again
    FailedToPlaceObjectAtSpecifiedLocation,
    /// script count changed
    ScriptCountChanged {
        /// script count before
        previous_script_count: u32,
        /// script count now
        current_script_count: u32,
        /// change
        change: i32,
    },
    /// the chat message to a multi-person chat is still being processed
    MultiPersonChatMessageStillBeingProcessed,
    /// the chat message to an im session that no longer exists is still being processed
    ChatMessageToNoLongerExistingImSessionStillBeingProcessed,
    /// the chat message to a conference is still being processed
    ConferenceChatMessageStillBeingProcessed {
        /// the name of the avatar whose conference it is
        avatar_name: String,
    },
    /// the group chat message is still being processed
    GroupChatMessageStillBeingProcessed {
        /// the name of the group
        group_name: String,
    },
    /// avatar has declined voice call
    AvatarDeclinedVoice {
        /// the avatar who declined our voice call
        avatar_name: String,
    },
    /// avatar is not available for voice call
    AvatarUnavailableForVoice {
        /// the avatar who was unavailable for our voice call
        avatar_name: String,
    },
    /// audio from a specific domain will always be played (on the audio stream)
    AudioFromDomainWillAlwaysBePlayed {
        /// the domain whose audio will always be played
        domain: String,
    },
    /// the object is not for sale
    ObjectNotForSale,
    /// cannot created requested inventory
    CanNotCreateRequestedInventory,
    /// link failed because pieces being too far apart
    LinkFailedDueToPieceDistance {
        /// link failed for this many pieces
        link_failed_pieces: Option<usize>,
        /// total selected pieces
        total_selected_pieces: Option<usize>,
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
    /// creating an object failed because the region is full
    CreateObjectFailedDueToFullRegion,
    /// your object has been returned to your inventory Lost and Found folder
    YourObjectHasBeenReturned {
        /// name of the returned object
        object_name: String,
        /// from parcel name
        parcel_name: String,
        /// at location
        location: sl_types::map::UnconstrainedLocation,
        /// due to parcel auto return?
        auto_return: bool,
    },
    /// permission to create an object denied
    PermissionToCreateObjectDenied,
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
    /// permission to change shape denied
    PermissionToChangeShapeDenied,
    /// permission to enter parcel denied
    PermissionToEnterParcelDenied,
    /// permission to enter parcel denied due to ban
    PermissionToEnterParcelDeniedDueToBan,
    /// we ejected an avatar
    EjectedAvatar,
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
    /// usage instruction for dice roll command
    DiceRollCommandUsageInstructions,
    /// dice roll result
    DiceRollResult {
        /// which dice roll (when multiple rolls were requested)
        roll_number: usize,
        /// how many faces on the dice rolled
        dice_faces: usize,
        /// roll result
        roll_result: usize,
    },
    /// dice roll result sum
    DiceRollResultSum {
        /// number of rolls
        roll_count: usize,
        /// how many faces on the dice rolled
        dice_faces: usize,
        /// result sum
        result_sum: usize,
    },
    /// texture info for object (followed by one or more of the below)
    TextureInfoForObject {
        /// name of the object
        object_name: String,
    },
    /// texture info for one face
    TextureInfoForFace {
        /// number of the face this line is about
        face_number: usize,
        /// width of the texture
        texture_width: u16,
        /// height of the texture
        texture_height: u16,
        /// type of texture, e.g. opaque, alpha
        texture_type: String,
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
    /// message with a link at the end (mostly announcements of events or
    /// similar message of the day style stuff)
    SystemMessageWithLink {
        /// the message before the link
        message: String,
        /// the link
        link: String,
    },
    /// Firestorm holiday wishes
    FirestormHolidayWishes {
        /// the message
        message: String,
    },
    /// Warning about phishing
    PhishingWarning {
        /// the message
        message: String,
    },
    /// Test MOTD
    TestMessageOfTheDay,
    /// Early Firestorm startup message
    EarlyFirestormStartupMessage {
        /// the message
        message: String,
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
        .map(|filename| SystemMessage::SavedSnapshot { filename })
        .or(just("Failed to save snapshot to ").ignore_then(
            take_until(just(": Directory does not exist.").ignored()).map(|(folder, ())| {
                SystemMessage::FailedToSaveSnapshotDueToMissingDestinationFolder {
                    folder: std::path::PathBuf::from(folder.into_iter().collect::<String>()),
                }
            }),
        ))
        .or(just("Failed to save snapshot to ").ignore_then(
            take_until(just(": Disk is full. ").ignored())
                .map(|(folder, ())| {
                    std::path::PathBuf::from(folder.into_iter().collect::<String>())
                })
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

/// parse a system message about the draw distance being set to a specific distance
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn draw_distance_set_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("Draw Distance set to ")
        .ignore_then(sl_types::map::distance_parser())
        .then_ignore(just('.'))
        .map(|distance| SystemMessage::DrawDistanceSet { distance })
}

/// parse a system message about the home position being set
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn home_position_set_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("Home position set.").to(SystemMessage::HomePositionSet)
}

/// parse a system message about land being divided
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn land_divided_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Land has been divided.").to(SystemMessage::LandDivided)
}

/// parse a system message about a failure to join land due to a region boundary
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn failed_to_join_land_due_to_region_boundary_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Selected land is not all in the same region.")
        .ignore_then(newline())
        .ignore_then(just(" Try selecting a smaller piece of land."))
        .to(SystemMessage::FailedToJoinLandDueToRegionBoundary)
}

/// parse a system message about offering a calling card to an avatar
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn offered_calling_card_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You have offered a calling card to ")
        .ignore_then(take_until(just('.')).map(|(vc, _)| vc.into_iter().collect::<String>()))
        .map(|recipient_avatar_name| SystemMessage::OfferedCallingCard {
            recipient_avatar_name,
        })
}

/// parse a system message about a saved attachment
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn attachment_saved_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Attachment has been saved").to(SystemMessage::AttachmentSavedMessage)
}

/// parse a system message about a sent payment
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn you_paid_for_object_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("You paid ")
        .ignore_then(sl_types::key::app_agent_or_group_uri_as_owner_key_parser())
        .then_ignore(whitespace())
        .then(sl_types::money::linden_amount_parser())
        .then_ignore(just(" for "))
        .then(take_until(just(".")).map(|(vc, _)| vc.into_iter().collect::<String>()))
        .map(
            |((seller, amount), object_name)| SystemMessage::YouPaidForObject {
                seller,
                amount,
                object_name,
            },
        )
}

/// parse a system message about a sent payment
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn sent_payment_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You paid ").ignore_then(
        sl_types::key::app_agent_or_group_uri_as_owner_key_parser()
            .then_ignore(whitespace())
            .then(sl_types::money::linden_amount_parser())
            .then(
                just(": ")
                    .ignore_then(any().repeated().collect::<String>())
                    .ignore_then(take_until(newline().or(end())).map(|(n, ())| Some(n)))
                    .or(just(".").map(|_| None)),
            )
            .map(
                |((recipient_key, amount), message)| SystemMessage::SentPayment {
                    recipient_key,
                    amount,
                    message: message.map(|n| n.into_iter().collect()),
                },
            ),
    )
}

/// parse a system message about a received payment
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn received_payment_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    sl_types::key::app_agent_or_group_uri_as_owner_key_parser()
        .then_ignore(just(" paid you "))
        .then(sl_types::money::linden_amount_parser())
        .then(
            just(": ")
                .ignore_then(any().repeated().collect::<String>())
                .ignore_then(take_until(newline().or(end())).map(|(n, ())| Some(n)))
                .or(just(".").map(|_| None)),
        )
        .map(
            |((sender_key, amount), message)| SystemMessage::ReceivedPayment {
                sender_key,
                amount,
                message: message.map(|n| n.into_iter().collect()),
            },
        )
}

/// parse a system message about paying to create a group
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn you_paid_to_create_a_group_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You paid ")
        .ignore_then(sl_types::key::app_agent_uri_as_agent_key_parser())
        .then_ignore(whitespace())
        .then(sl_types::money::linden_amount_parser())
        .then_ignore(just(" to create a group."))
        .map(
            |(payment_recipient, amount)| SystemMessage::YouPaidToCreateGroup {
                payment_recipient,
                amount,
            },
        )
}

/// parse a system message about paying to join a group
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn you_paid_to_join_group_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You paid ")
        .ignore_then(sl_types::key::app_group_uri_as_group_key_parser())
        .then_ignore(whitespace())
        .then(sl_types::money::linden_amount_parser())
        .then_ignore(just(" to join a group."))
        .map(
            |(joined_group, join_fee)| SystemMessage::YouPaidToJoinGroup {
                joined_group,
                join_fee,
            },
        )
}

/// parse a system message about paying for land
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn you_paid_for_land_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("You paid ")
        .ignore_then(sl_types::key::app_agent_or_group_uri_as_owner_key_parser())
        .then_ignore(whitespace())
        .then(sl_types::money::linden_amount_parser())
        .then_ignore(just(" for a parcel of land."))
        .map(
            |(previous_land_owner, amount)| SystemMessage::YouPaidForLand {
                previous_land_owner,
                amount,
            },
        )
}

/// parse a system message about a failure to pay
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn failed_to_pay_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You failed to pay ")
        .ignore_then(sl_types::key::app_agent_or_group_uri_as_owner_key_parser())
        .then_ignore(whitespace())
        .then(sl_types::money::linden_amount_parser())
        .then_ignore(just('.'))
        .map(|(payment_recipient, amount)| SystemMessage::FailedToPay {
            payment_recipient,
            amount,
        })
}

/// parse a system message about an object being granted permission to take L$
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn object_granted_permission_to_take_money_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just('\'')
        .ignore_then(
            take_until(just("', an object owned by '"))
                .map(|(vc, _)| vc.into_iter().collect::<String>()),
        )
        .then(take_until(just("', located in ")).map(|(vc, _)| vc.into_iter().collect::<String>()))
        .then(
            just("(unknown region) at ")
                .to(None)
                .or(take_until(just(" at ")).try_map(|(vc, _), span| {
                    Ok(Some(
                        sl_types::map::RegionName::try_new(vc.into_iter().collect::<String>())
                            .map_err(|err| {
                                Simple::custom(span, format!("Error creating region name: {err:?}"))
                            })?,
                    ))
                })),
        )
        .then(
            just("(unknown position)")
                .to(None)
                .or(sl_types::utils::f32_parser()
                    .then_ignore(just(", "))
                    .then(sl_types::utils::f32_parser())
                    .then_ignore(just(','))
                    .then(sl_types::utils::f32_parser())
                    .map(|((x, y), z)| Some(sl_types::map::RegionCoordinates::new(x, y, z)))),
        )
        .then_ignore(just(
            ", has been granted permission to: Take Linden dollars (L$) from you.",
        ))
        .map(
            |(((object_name, owner_name), object_region), object_location)| {
                SystemMessage::ObjectGrantedPermissionToTakeMoney {
                    object_name,
                    owner_name,
                    object_region,
                    object_location,
                }
            },
        )
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
            .ignore_then(take_until(just("'.")).map(|(vc, _)| vc.into_iter().collect::<String>()))
            .map(|group_name| SystemMessage::LeftGroup { group_name }))
}

/// parse a system message about the inability to invite a user to a group
/// you yourself are not a member of
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_invite_user_due_to_missing_group_membership_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to invite user because you are not in that group.")
        .to(SystemMessage::UnableToInviteUserDueToMissingGroupMembership)
}

/// parse a system message about the inability to invite a user to a group
/// due to a difference in limited estate between user and group
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_invite_user_due_to_differing_limited_estate_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to invite users because at least one user is in a different")
        .ignore_then(newline())
        .ignore_then(just(" limited estate than the group."))
        .to(SystemMessage::UnableToInviteUserToGroupDueToDifferingLimitedEstate)
}

/// parse a system message about the inability to load a notecard
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_load_notecard_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to load the notecard.")
        .then_ignore(newline())
        .then_ignore(whitespace())
        .then(just("Please try again."))
        .to(SystemMessage::UnableToLoadNotecard)
}

/// parse a system message about the inability to load a gesture
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_load_gesture_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to load gesture ")
        .ignore_then(
            take_until(just('.').then(end())).map(|(vc, _)| vc.into_iter().collect::<String>()),
        )
        .map(|gesture_name| SystemMessage::UnableToLoadGesture { gesture_name })
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
        .ignore_then(sl_types::map::url_unconstrained_location_parser())
        .try_map(|origin, _span: std::ops::Range<usize>| {
            Ok(SystemMessage::TeleportCompleted { origin })
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
            Ok(SystemMessage::NowPlaying { song_name })
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
            Ok(SystemMessage::RegionRestart)
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
        .then(sl_types::key::app_agent_or_group_uri_as_owner_key_parser())
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
        .then(sl_types::map::url_unconstrained_location_parser())
        .then_ignore(just(" )."))
        .map(
            |(
                (((giving_object_name, _), giving_object_owner), (given_object_name, _)),
                giving_object_location,
            )| {
                SystemMessage::ObjectGaveObject {
                    giving_object_name: giving_object_name.into_iter().collect(),
                    giving_object_owner,
                    given_object_name: given_object_name.into_iter().collect(),
                    giving_object_location,
                }
            },
        )
}

/// parse a system message about an object giving the current avatar a folder
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn object_gave_folder_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>>
{
    just("An object named [")
        .ignore_then(sl_types::viewer_uri::viewer_app_objectim_uri_parser())
        .then_ignore(whitespace())
        .then(
            take_until(just("] gave you this folder: '"))
                .map(|(vc, _)| vc.into_iter().collect::<String>()),
        )
        .then(take_until(just('\'')).map(|(vc, _)| vc.into_iter().collect::<String>()))
        .try_map(
            |((app_objectim_uri, giving_object_link_label), folder_name), span| {
                match app_objectim_uri {
                    sl_types::viewer_uri::ViewerUri::ObjectInstantMessage {
                        object_key,
                        object_name,
                        owner,
                        location,
                    } => Ok(SystemMessage::ObjectGaveFolder {
                        giving_object_key: object_key,
                        giving_object_name: object_name,
                        giving_object_owner: owner,
                        giving_object_location: location,
                        giving_object_link_label,
                        folder_name,
                    }),
                    _ => Err(Simple::custom(
                        span,
                        "Unexpected type of viewer URI in object gave folder message parser",
                    )),
                }
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
                Ok(SystemMessage::AvatarGaveObject {
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
pub fn declined_given_object_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You decline '")
        .ignore_then(
            take_until(just("'  ( http://slurl.com/secondlife/").ignored())
                .map(|(vc, ())| vc.into_iter().collect::<String>()),
        )
        .then(sl_types::map::url_unconstrained_location_parser())
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

/// parse a system message asking to select residents to share with
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn select_residents_to_share_with_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Select residents to share with.").to(SystemMessage::SelectResidentsToShareWith)
}

/// parse a system message about items being successfully shared
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn items_successfully_shared_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Items successfully shared.").to(SystemMessage::ItemsSuccessfullyShared)
}

/// parse a system message about a modified search query
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn modified_search_query_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
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
pub fn doubleclick_teleport_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("DoubleClick Teleport enabled.")
        .to(SystemMessage::DoubleClickTeleport { enabled: true })
        .or(just("DoubleClick Teleport disabled.")
            .to(SystemMessage::DoubleClickTeleport { enabled: false }))
}

/// parse a system message about enabling or disabling of always run
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn always_run_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Always Run enabled.")
        .to(SystemMessage::AlwaysRun { enabled: true })
        .or(just("Always Run disabled.").to(SystemMessage::AlwaysRun { enabled: false }))
}

/// parse a system message about being added as an estate manager
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn added_as_estate_manager_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You have been added as an estate manager.").to(SystemMessage::AddedAsEstateManager)
}

/// parse a system message about the LSL viewer bridge
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn bridge_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    choice([
        just("Creating the bridge. This might take a moment, please wait.").to(SystemMessage::CreatingBridge).boxed(),
        just("Creating the bridge. This might take a few moments, please wait").to(SystemMessage::CreatingBridge).boxed(),
        just("Bridge created.").to(SystemMessage::BridgeCreated).boxed(),
        just("Bridge creation in process, cannot start another. Please wait a few minutes before trying again.").to(SystemMessage::BridgeCreationInProgress).boxed(),
        just("Bridge object not found. Can't proceed with creation, exiting.").to(SystemMessage::BridgeObjectNotFoundCantProceedWithCreation).boxed(),
        just("Bridge failed to attach. This is not the current bridge version. Please use the Firestorm 'Avatar/Avatar Health/Recreate Bridge' menu option to recreate the bridge.").to(SystemMessage::BridgeFailedToAttach).boxed(),
        just("Bridge failed to attach. Something else was using the bridge attachment point. Please try to recreate the bridge.").to(SystemMessage::BridgeFailedToAttachDueToBridgeAttachmentPointInUse).boxed(),
        just("Bridge failed to attach. Something else was using the bridge attachment point. Please use the Firestorm 'Avatar/Avatar Health/Recreate Bridge' menu option to recreate the bridge.").to(SystemMessage::BridgeFailedToAttachDueToBridgeAttachmentPointInUse).boxed(),
        just("Bridge not created. The bridge couldn't be found in inventory. Please use the Firestorm 'Avatar/Avatar Health/Recreate Bridge' menu option to recreate the bridge.").to(SystemMessage::BridgeNotCreated).boxed(),
        just("Bridge detached.").to(SystemMessage::BridgeDetached).boxed(),
    ])
}

/// parse a system message about a failure to place an object a a specified
/// location
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn failed_to_place_object_at_specified_location_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Failed to place object at specified location.  Please try again.")
        .to(SystemMessage::FailedToPlaceObjectAtSpecifiedLocation)
}

/// parse a system message about a changed script count in the current region
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn region_script_count_change_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
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
                                        "Could not parse previous script count ({previous_script_count}) as u32: {err:?}"
                                    ),
                                )
                            })?;
                        let current_span = span.clone();
                        let current_script_count = current_script_count.parse().map_err(|err| {
                            Simple::custom(
                                current_span,
                                format!(
                                    "Could not parse current script count ({current_script_count}) as u32: {err:?}"
                                ),
                            )
                        })?;
                        let diff_span = span.clone();
                        let diff: i32 = diff.parse().map_err(|err| {
                            Simple::custom(
                                diff_span,
                                format!(
                                    "Could not parse changed script count ({diff}) as i32: {err:?}"
                                ),
                            )
                        })?;
                        let change = match sign {
                            '+' => diff,
                            '-' => {
                                #[expect(clippy::arithmetic_side_effects, reason = "this just switches the sign of a positive value to a negative one but only the opposite switch is panic-prone at i32::MIN")]
                                -diff
                            }
                            c => {
                                return Err(Simple::custom(
                                    span,
                                    format!("Unexpected sign character for script change: {c}"),
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

/// parse a system message about a chat message still being processed
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn chat_message_still_being_processed_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("The message sent to ")
        .ignore_then(
            choice([
                just("Multi-person chat is still being processed.").to(SystemMessage::MultiPersonChatMessageStillBeingProcessed).boxed(),
                just("(IM Session Doesn't Exist) is still being processed.").to(SystemMessage::ChatMessageToNoLongerExistingImSessionStillBeingProcessed).boxed(),
                just("Conference with ").ignore_then(take_until(just(" is still being processed.")).map(|(vc, _)| SystemMessage::ConferenceChatMessageStillBeingProcessed { avatar_name: vc.into_iter().collect::<String>() })).boxed(),
                take_until(just(" is still being processed.").ignored()).map(|(vc, ())| vc.into_iter().collect::<String>())
                .map(|group_name| {
                    SystemMessage::GroupChatMessageStillBeingProcessed {
                        group_name,
                    }
                }).boxed(),
            ])
        )
        .then_ignore(newline())
        .then_ignore(whitespace())
        .then_ignore(just("If the message does not appear in the next few minutes, it may have been dropped by the server."))
}

/// parse a system message about an avatar declining a voice call
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn avatar_declined_voice_call_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    take_until(just(
        "has declined your call.  You will now be reconnected to Nearby Voice Chat.",
    ))
    .map(|(vc, _)| SystemMessage::AvatarDeclinedVoice {
        avatar_name: vc.into_iter().collect::<String>(),
    })
}

/// parse a system message about an avatar being unavailable to take our voice call
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn avatar_unavailable_for_voice_call_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    take_until(just(
        "is not available to take your call.  You will now be reconnected to Nearby Voice Chat.",
    ))
    .map(|(vc, _)| SystemMessage::AvatarUnavailableForVoice {
        avatar_name: vc.into_iter().collect::<String>(),
    })
}

/// parse a system message about audio from a domain always being played
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn audio_from_domain_will_always_be_played_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Audio from the domain ").ignore_then(take_until(just(" will always be played.")).map(
        |(vc, _)| SystemMessage::AudioFromDomainWillAlwaysBePlayed {
            domain: vc.into_iter().collect::<String>(),
        },
    ))
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

/// parse a system message cannot create requested inventory
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn can_not_create_requested_inventory_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Cannot create requested inventory.").to(SystemMessage::CanNotCreateRequestedInventory)
}

/// parse a system message about a failed link due to piece distance
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn link_failed_due_to_piece_distance_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Link failed -- Unable to link any pieces - pieces are too far apart.")
        .to(SystemMessage::LinkFailedDueToPieceDistance {
            link_failed_pieces: None,
            total_selected_pieces: None,
        })
        .or(just("Link failed -- Unable to link ").ignore_then(
            usize_parser()
                .then_ignore(just(" of the "))
                .then(usize_parser())
                .then_ignore(just(" selected pieces - pieces are too far apart."))
                .map(|(link_failed_pieces, total_selected_pieces)| {
                    SystemMessage::LinkFailedDueToPieceDistance {
                        link_failed_pieces: Some(link_failed_pieces),
                        total_selected_pieces: Some(total_selected_pieces),
                    }
                }),
        ))
}

/// parse a system message about the failure to rez an object due to a full
/// parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn rezzing_object_failed_due_to_full_parcel_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rez object '").ignore_then(
        take_until(just("' at ").ignored())
            .map(|(vc, ())| vc.into_iter().collect::<String>())
            .then(sl_types::map::region_coordinates_parser())
            .then_ignore(just(" on parcel '"))
            .then(
                take_until(just("' in region ").ignored())
                    .map(|(vc, ())| vc.into_iter().collect::<String>()),
            )
            .then(
                take_until(just(" because the parcel is too full").ignored())
                    .map(|(vc, ())| vc.into_iter().collect::<String>())
                    .try_map(|region_name, span| {
                        sl_types::map::RegionName::try_new(&region_name).map_err(|err| {
                            Simple::custom(
                                span,
                                format!(
                                    "Could not turn parsed region name ({region_name}) into RegionName: {err:?}"
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

/// parse a system message about the failure to create an object due to a full
/// region
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn create_object_failed_due_to_full_region_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to create requested object. The region is full.")
        .to(SystemMessage::CreateObjectFailedDueToFullRegion)
}

/// parse a system message about an object being returned to your inventory
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn your_object_has_been_returned_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Your object '")
        .ignore_then(
            take_until(just(
                "' has been returned to your inventory Lost and Found folder from parcel '",
            ))
            .map(|(vc, _)| vc.into_iter().collect::<String>()),
        )
        .then(take_until(just("' at ")).map(|(vc, _)| vc.into_iter().collect::<String>()))
        .then(sl_types::map::region_name_parser())
        .then_ignore(whitespace())
        .then(sl_types::utils::i16_parser())
        .then_ignore(just(", "))
        .then(sl_types::utils::i16_parser())
        .then(
            just(" due to parcel auto return.")
                .to(true)
                .or(just('.').to(false)),
        )
        .map(
            |(((((object_name, parcel_name), region_name), x), y), auto_return)| {
                SystemMessage::YourObjectHasBeenReturned {
                    object_name,
                    parcel_name,
                    location: sl_types::map::UnconstrainedLocation::new(region_name, x, y, 0),
                    auto_return,
                }
            },
        )
}

/// parse a system message about the denial of permission to create an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_create_object_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You cannot create objects here.  The owner of this land does not allow it.  Use the land tool to see land ownership.").to(SystemMessage::PermissionToCreateObjectDenied)
}

/// parse a system message about the denial of permission to rez an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_rez_object_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rez object '")
        .ignore_then(
            take_until(just("' at ").ignored()).map(|(vc, ())| vc.into_iter().collect::<String>())
            .then(sl_types::map::region_coordinates_parser())
            .then_ignore(just(" on parcel '"))
            .then(take_until(just("' in region ").ignored()).map(|(vc, ())| vc.into_iter().collect::<String>()))
            .then(take_until(just(" because the owner of this land does not allow it.  Use the land tool to see land ownership.").ignored()).map(|(vc, ())| vc.into_iter().collect::<String>()).try_map(|region_name, span| {
                sl_types::map::RegionName::try_new(&region_name).map_err(|err| Simple::custom(span, format!("Could not turn parsed region name ({region_name}) into RegionName: {err:?}")))
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
pub fn permission_to_reposition_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't reposition -- permission denied").to(SystemMessage::PermissionToRepositionDenied)
}

/// parse a system message about the denial of permission to rotate an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_rotate_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rotate -- permission denied").to(SystemMessage::PermissionToRotateDenied)
}

/// parse a system message about the denial of permission to rescale an object
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_rescale_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Can't rescale -- permission denied").to(SystemMessage::PermissionToRescaleDenied)
}

/// parse a system message about the denial of permission to unlink an object
/// because build permissions are missing on at least one parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_unlink_denied_due_to_missing_parcel_build_permissions_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Failed to unlink because you do not have permissions to build on all parcels")
        .to(SystemMessage::PermissionToUnlinkDeniedDueToMissingParcelBuildPermissions)
}

/// parse a system message about the denial of permission to view a script
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_view_script_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Insufficient permissions to view the script.")
        .to(SystemMessage::PermissionToViewScriptDenied)
}

/// parse a system message about the denial of permission to view a notecard
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_view_notecard_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You do not have permission to view this notecard.")
        .to(SystemMessage::PermissionToViewNotecardDenied)
}

/// parse a system message about the denial of permission to change a shape
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_change_shape_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("You are not allowed to change this shape.")
        .to(SystemMessage::PermissionToChangeShapeDenied)
}

/// parse a system message about the denial of permission to enter a parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_enter_parcel_denied_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Cannot enter parcel, you are not on the access list.")
        .to(SystemMessage::PermissionToEnterParcelDenied)
}

/// parse a system message about the denial of permission to enter a parcel due to ban
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn permission_to_enter_parcel_denied_due_to_ban_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Cannot enter parcel, you have been banned.")
        .to(SystemMessage::PermissionToEnterParcelDeniedDueToBan)
}

/// parse a system message about ejecting an avatar from a parcel
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn avatar_ejected_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Avatar ejected.").to(SystemMessage::EjectedAvatar)
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
pub fn only_group_members_can_visit_this_area_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Only members of a certain group can visit this area.")
        .to(SystemMessage::OnlyGroupMembersCanVisitThisArea)
}

/// parse a system message about teleports being RLV restricted
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_teleport_due_to_rlv_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Unable to initiate teleport due to RLV restrictions")
        .to(SystemMessage::UnableToTeleportDueToRlv)
}

/// parse a system message about opening textures being RLV restricted
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn unable_to_open_texture_due_to_rlv_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
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
pub fn blocked_untrusted_browser_slurl_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("A SLurl was received from an untrusted browser and has been blocked for your security")
        .to(SystemMessage::BlockedUntrustedBrowserSlurl)
}

/// parse a system message about a grid status error about an invalid message format
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn grid_status_error_invalid_message_format_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("SL Grid Status error: Invalid message format. Try again later.")
        .to(SystemMessage::GridStatusErrorInvalidMessageFormat)
}

/// parse a system message about a script info object being invalid or out of range
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn script_info_object_invalid_or_out_of_range_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
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
            .map(|(vc, ())| vc.into_iter().collect::<String>())
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
pub fn extended_script_info_message_parser()
-> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Object ID: ")
        .ignore_then(sl_types::key::object_key_parser())
        .then_ignore(newline())
        .then_ignore(just(" Description:"))
        .then_ignore(just(" ").or_not())
        .then(just("(No Description)").then_ignore(newline()).to(None).or(
            take_until(newline().ignored()).map(|(vc, ())| Some(vc.into_iter().collect::<String>())),
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

/// parse a system message about dice rolls
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn dice_roll_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    choice([
        just("You must provide positive values for dice (max 100) and faces (max 1000).")
            .to(SystemMessage::DiceRollCommandUsageInstructions)
            .boxed(),
        just('#')
            .ignore_then(usize_parser())
            .then_ignore(whitespace())
            .then_ignore(just("1d"))
            .then(usize_parser())
            .then_ignore(just(":"))
            .then_ignore(whitespace())
            .then(usize_parser())
            .then_ignore(just('.'))
            .map(
                |((roll_number, dice_faces), roll_result)| SystemMessage::DiceRollResult {
                    roll_number,
                    dice_faces,
                    roll_result,
                },
            )
            .boxed(),
        just("Total result for ")
            .ignore_then(usize_parser())
            .then_ignore(just('d'))
            .then(usize_parser())
            .then_ignore(just(':'))
            .then_ignore(whitespace())
            .then(usize_parser())
            .then_ignore(just('.'))
            .map(
                |((roll_count, dice_faces), result_sum)| SystemMessage::DiceRollResultSum {
                    roll_count,
                    dice_faces,
                    result_sum,
                },
            )
            .boxed(),
    ])
}

/// parse a system message about object textures on faces
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn texture_info_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    choice([
        just("Texture info for: ")
            .ignore_then(take_until(newline().or(end())).map(|(vc, ())| {
                SystemMessage::TextureInfoForObject {
                    object_name: vc.into_iter().collect::<String>(),
                }
            }))
            .boxed(),
        sl_types::utils::u16_parser()
            .then_ignore(just('x'))
            .then(sl_types::utils::u16_parser())
            .then_ignore(whitespace())
            .then(just("opaque").or(just("alpha")))
            .then_ignore(just(" on face "))
            .then(usize_parser())
            .map(
                |(((texture_width, texture_height), texture_type), face_number)| {
                    SystemMessage::TextureInfoForFace {
                        face_number,
                        texture_width,
                        texture_height,
                        texture_type: texture_type.to_owned(),
                    }
                },
            )
            .boxed(),
    ])
}

/// parse a system message by the Firestorm developers
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn firestorm_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    just("Firestorm ").ignore_then(
        take_until(just("!").ignored())
            .map(|(message_type, ())| message_type.into_iter().collect::<String>())
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
                    .map(|(vc, ())| vc.into_iter().collect::<String>()),
            )
            .then(
                take_until(just(' ').ignored()).map(|(vc, ())| vc.into_iter().collect::<String>()),
            )
            .then_ignore(just("]"))
            .map(
                |(((title, scheduled), body), url_fragment)| SystemMessage::GridStatusEvent {
                    title,
                    scheduled,
                    body,
                    incident_url: format!(
                        "https://status.secondlifegird.net/incidents/{url_fragment}"
                    ),
                },
            ),
    )
}

/// parse a Second Life system message
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn system_message_parser() -> impl Parser<char, SystemMessage, Error = Simple<char>> {
    choice([
        snapshot_saved_message_parser().boxed(),
        attachment_saved_message_parser().boxed(),
        draw_distance_set_message_parser().boxed(),
        home_position_set_message_parser().boxed(),
        land_divided_message_parser().boxed(),
        failed_to_join_land_due_to_region_boundary_message_parser().boxed(),
        offered_calling_card_message_parser().boxed(),
        you_paid_for_object_message_parser().boxed(),
        you_paid_to_create_a_group_message_parser().boxed(),
        you_paid_to_join_group_message_parser().boxed(),
        you_paid_for_land_message_parser().boxed(),
        failed_to_pay_message_parser().boxed(),
        object_granted_permission_to_take_money_parser().boxed(),
        sent_payment_message_parser().boxed(),
        received_payment_message_parser().boxed(),
        group_membership_message_parser().boxed(),
        unable_to_invite_user_due_to_missing_group_membership_message_parser().boxed(),
        unable_to_invite_user_due_to_differing_limited_estate_message_parser().boxed(),
        unable_to_load_notecard_message_parser().boxed(),
        unable_to_load_gesture_message_parser().boxed(),
        teleport_completed_message_parser().boxed(),
        now_playing_message_parser().boxed(),
        region_restart_message_parser().boxed(),
        object_gave_object_message_parser().boxed(),
        object_gave_folder_message_parser().boxed(),
        declined_given_object_message_parser().boxed(),
        select_residents_to_share_with_message_parser().boxed(),
        items_successfully_shared_message_parser().boxed(),
        modified_search_query_message_parser().boxed(),
        avatar_gave_object_message_parser().boxed(),
        simulator_version_message_parser().boxed(),
        renamed_avatar_message_parser().boxed(),
        doubleclick_teleport_message_parser().boxed(),
        always_run_message_parser().boxed(),
        added_as_estate_manager_message_parser().boxed(),
        bridge_message_parser().boxed(),
        failed_to_place_object_at_specified_location_message_parser().boxed(),
        region_script_count_change_message_parser().boxed(),
        chat_message_still_being_processed_message_parser().boxed(),
        avatar_declined_voice_call_message_parser().boxed(),
        avatar_unavailable_for_voice_call_message_parser().boxed(),
        audio_from_domain_will_always_be_played_message_parser().boxed(),
        object_not_for_sale_message_parser().boxed(),
        can_not_create_requested_inventory_message_parser().boxed(),
        link_failed_due_to_piece_distance_message_parser().boxed(),
        rezzing_object_failed_due_to_full_parcel_message_parser().boxed(),
        create_object_failed_due_to_full_region_message_parser().boxed(),
        your_object_has_been_returned_message_parser().boxed(),
        permission_to_create_object_denied_message_parser().boxed(),
        permission_to_rez_object_denied_message_parser().boxed(),
        permission_to_reposition_denied_message_parser().boxed(),
        permission_to_rotate_denied_message_parser().boxed(),
        permission_to_rescale_denied_message_parser().boxed(),
        permission_to_unlink_denied_due_to_missing_parcel_build_permissions_message_parser()
            .boxed(),
        permission_to_view_script_denied_message_parser().boxed(),
        permission_to_view_notecard_denied_message_parser().boxed(),
        permission_to_change_shape_denied_message_parser().boxed(),
        permission_to_enter_parcel_denied_message_parser().boxed(),
        permission_to_enter_parcel_denied_due_to_ban_message_parser().boxed(),
        avatar_ejected_message_parser().boxed(),
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
        dice_roll_message_parser().boxed(),
        texture_info_message_parser().boxed(),
        firestorm_message_parser().boxed(),
        grid_status_event_message_parser().boxed(),
        take_until(just("https").or(just("http")).or(just("Http")))
            .map(|(message, scheme)| (message.into_iter().collect::<String>(), scheme))
            .then(take_until(newline().or(end())).map(|(vc, ())| vc.into_iter().collect::<String>()))
            .map(
                |((message, scheme), rest_of_url)| SystemMessage::SystemMessageWithLink {
                    message,
                    link: format!("{scheme}://{rest_of_url}"),
                },
            )
            .boxed(),
        take_until(just("www."))
            .map(|(message, subdomain)| (message.into_iter().collect::<String>(), subdomain))
            .then(take_until(newline().or(end())).map(|(vc, ())| vc.into_iter().collect::<String>()))
            .map(
                |((message, subdomain), rest_of_url)| SystemMessage::SystemMessageWithLink {
                    message,
                    link: format!("{subdomain}{rest_of_url}"),
                },
            )
            .boxed(),
        any()
            .repeated()
            .collect::<String>()
            .map(|message| {
                if message.contains("Firestorm") && (message.contains("holiday") || message.contains("Happy New Year")) {
                    SystemMessage::FirestormHolidayWishes { message }
                } else if message.contains("phishing") {
                    SystemMessage::PhishingWarning { message }
                } else if message == "This is a test version of Firestorm. If this were an actual release version, a real message of the day would be here. This is only a test." {
                    SystemMessage::TestMessageOfTheDay
                } else if (message.ends_with("...") && (message.starts_with("Loading") || message.starts_with("Initializing") || message.starts_with("Downloading") || message.starts_with("Verifying") || message.starts_with("Loading") || message.starts_with("Connecting") || message.starts_with("Decoding") || message.starts_with("Waiting"))) || message == "Welcome to Advertisement-Free Firestorm" || message.starts_with("Logging in") {
                    SystemMessage::EarlyFirestormStartupMessage { message }
                } else if message.contains("wiki.phoenixviewer.com/firestorm_classes") {
                    SystemMessage::FirestormMessage {
                        message_type: "Classes".to_string(),
                        message,
                    }
                } else if message.contains("BETA TESTERS") || message.contains("Beta Testers") {
                    SystemMessage::FirestormMessage {
                        message_type: "Beta Test".to_string(),
                        message,
                    }
                } else {
                    SystemMessage::OtherSystemMessage { message }
                }
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
            Ok(SystemMessage::TeleportCompleted {
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
            Ok(SystemMessage::TeleportCompleted {
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
