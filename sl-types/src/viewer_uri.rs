//! Viewer URI related types
//!
//! see <https://wiki.secondlife.com/wiki/Viewer_URI_Name_Space>

#[cfg(feature = "chumsky")]
use chumsky::{Parser, prelude::just};
#[cfg(feature = "chumsky")]
use std::ops::Deref as _;

#[cfg(feature = "chumsky")]
use crate::utils::url_text_component_parser;

/// represents the various script trigger modes for the script_trigger_lbutton
/// key binding
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, strum::FromRepr, strum::EnumIs)]
pub enum ScriptTriggerMode {
    /// "first_person" or 0
    FirstPerson = 0,
    /// "third_person" or 1
    ThirdPerson = 1,
    /// "edit_avatar" or 2
    EditAvatar = 2,
    /// "sitting" or 3
    Sitting = 3,
}

/// parse script trigger mode
///
/// # Errors
///
/// returns and error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn script_trigger_mode_parser<'src>()
-> impl Parser<'src, &'src str, ScriptTriggerMode, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("first_person")
        .to(ScriptTriggerMode::FirstPerson)
        .or(just("third_person").to(ScriptTriggerMode::ThirdPerson))
        .or(just("edit_avatar").to(ScriptTriggerMode::EditAvatar))
        .or(just("sitting").to(ScriptTriggerMode::Sitting))
        .labelled("script trigger mode")
}

impl std::fmt::Display for ScriptTriggerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FirstPerson => write!(f, "first_person"),
            Self::ThirdPerson => write!(f, "third_person"),
            Self::EditAvatar => write!(f, "edit_avatar"),
            Self::Sitting => write!(f, "sitting"),
        }
    }
}

/// error when trying to parse a string as a ScriptTriggerMode
#[derive(Debug, Clone)]
pub struct ScriptTriggerModeParseError {
    /// the value that could not be parsed
    value: String,
}

impl std::fmt::Display for ScriptTriggerModeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse as ScriptTriggerMode: {}", self.value)
    }
}

impl std::error::Error for ScriptTriggerModeParseError {}

impl std::str::FromStr for ScriptTriggerMode {
    type Err = ScriptTriggerModeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "first_person" | "0" => Ok(Self::FirstPerson),
            "third_person" | "1" => Ok(Self::ThirdPerson),
            "edit_avatar" | "2" => Ok(Self::EditAvatar),
            "sitting" | "3" => Ok(Self::Sitting),
            _ => Err(ScriptTriggerModeParseError {
                value: s.to_owned(),
            }),
        }
    }
}

/// represents a Viewer URI
#[derive(Debug, Clone, PartialEq, Eq, strum::EnumIs)]
pub enum ViewerUri {
    /// a link to this location
    Location(crate::map::Location),
    /// opens the agent profile
    AgentAbout(crate::key::AgentKey),
    /// displays the info dialog for the agent
    AgentInspect(crate::key::AgentKey),
    /// starts an IM session with the agent
    AgentInstantMessage(crate::key::AgentKey),
    /// displays teleport offer dialog for the agent
    AgentOfferTeleport(crate::key::AgentKey),
    /// displays pay resident dialog
    AgentPay(crate::key::AgentKey),
    /// displays friendship offer dialog
    AgentRequestFriend(crate::key::AgentKey),
    /// adds agent to block list
    AgentMute(crate::key::AgentKey),
    /// removes agent from block list
    AgentUnmute(crate::key::AgentKey),
    /// replaces the URL with the agent's display and user names
    AgentCompleteName(crate::key::AgentKey),
    /// replaces the URL with the agent's display name
    AgentDisplayName(crate::key::AgentKey),
    /// replaces the URL with the agent's username
    AgentUsername(crate::key::AgentKey),
    /// show appearance
    AppearanceShow,
    /// request a L$ balance update from the server
    BalanceRequest,
    /// send a chat message to the given channel, won't work with DEBUG_CHANNEL
    Chat {
        /// the channel to send the message on, can not be DEBUG_CHANNEL
        channel: crate::chat::ChatChannel,
        /// the text to send
        text: String,
    },
    /// open a floater describing the classified ad
    ClassifiedAbout(crate::key::ClassifiedKey),
    /// open a floater describing the event
    EventAbout(crate::key::EventKey),
    /// open a floater describing the experience
    ExperienceProfile(crate::key::ExperienceKey),
    /// open the group profile
    GroupAbout(crate::key::GroupKey),
    /// displays the info dialog for the group
    GroupInspect(crate::key::GroupKey),
    /// open the create group dialog
    GroupCreate,
    /// open the group list to which the current avatar belongs
    GroupListShow,
    /// open help
    Help {
        /// optional help topic
        help_query: Option<String>,
    },
    /// offer inventory
    InventorySelect(crate::key::InventoryKey),
    /// show inventory
    InventoryShow,
    /// key binding
    KeyBindingMovementWalkTo,
    /// key binding
    KeyBindingMovementTeleportTo,
    /// key binding
    KeyBindingMovementPushForward,
    /// key binding
    KeyBindingMovementPushBackward,
    /// key binding
    KeyBindingMovementTurnLeft,
    /// key binding
    KeyBindingMovementTurnRight,
    /// key binding
    KeyBindingMovementSlideLeft,
    /// key binding
    KeyBindingMovementSlideRight,
    /// key binding
    KeyBindingMovementJump,
    /// key binding
    KeyBindingMovementPushDown,
    /// key binding
    KeyBindingMovementRunForward,
    /// key binding
    KeyBindingMovementRunBackward,
    /// key binding
    KeyBindingMovementRunLeft,
    /// key binding
    KeyBindingMovementRunRight,
    /// key binding
    KeyBindingMovementToggleRun,
    /// key binding
    KeyBindingMovementToggleFly,
    /// key binding
    KeyBindingMovementToggleSit,
    /// key binding
    KeyBindingMovementStopMoving,
    /// key binding
    KeyBindingCameraLookUp,
    /// key binding
    KeyBindingCameraLookDown,
    /// key binding
    KeyBindingCameraMoveForward,
    /// key binding
    KeyBindingCameraMoveBackward,
    /// key binding
    KeyBindingCameraMoveForwardFast,
    /// key binding
    KeyBindingCameraMoveBackwardFast,
    /// key binding
    KeyBindingCameraSpinOver,
    /// key binding
    KeyBindingCameraSpinUnder,
    /// key binding
    KeyBindingCameraPanUp,
    /// key binding
    KeyBindingCameraPanDown,
    /// key binding
    KeyBindingCameraPanLeft,
    /// key binding
    KeyBindingCameraPanRight,
    /// key binding
    KeyBindingCameraPanIn,
    /// key binding
    KeyBindingCameraPanOut,
    /// key binding
    KeyBindingCameraSpinAroundCounterClockwise,
    /// key binding
    KeyBindingCameraSpinAroundClockwise,
    /// key binding
    KeyBindingCameraMoveForwardSitting,
    /// key binding
    KeyBindingCameraMoveBackwardSitting,
    /// key binding
    KeyBindingCameraSpinOverSitting,
    /// key binding
    KeyBindingCameraSpinUnderSitting,
    /// key binding
    KeyBindingCameraSpinAroundCounterClockwiseSitting,
    /// key binding
    KeyBindingCameraSpinAroundClockwiseSitting,
    /// key binding
    KeyBindingEditingAvatarSpinCounterClockwise,
    /// key binding
    KeyBindingEditingAvatarSpinClockwise,
    /// key binding
    KeyBindingEditingAvatarSpinOver,
    /// key binding
    KeyBindingEditingAvatarSpinUnder,
    /// key binding
    KeyBindingEditingAvatarMoveForward,
    /// key binding
    KeyBindingEditingAvatarMoveBackward,
    /// key binding
    KeyBindingSoundAndMediaTogglePauseMedia,
    /// key binding
    KeyBindingSoundAndMediaToggleEnableMedia,
    /// key binding
    KeyBindingSoundAndMediaVoiceFollowKey,
    /// key binding
    KeyBindingSoundAndMediaToggleVoice,
    /// key binding
    KeyBindingStartChat,
    /// key binding
    KeyBindingStartGesture,
    /// key binding
    KeyBindingScriptTriggerLButton(ScriptTriggerMode),
    /// login on launch
    Login {
        /// account first name
        first_name: String,
        /// account last name
        last_name: String,
        /// secure session id
        session: String,
        /// login location
        login_location: Option<String>,
    },
    /// track a friend with the permission on the world map
    MapTrackAvatar(crate::key::FriendKey),
    /// display an info dialog for the object sending this message
    ObjectInstantMessage {
        /// key of the object
        object_key: crate::key::ObjectKey,
        /// name of the object
        object_name: String,
        /// owner of the object
        owner: crate::key::OwnerKey,
        /// object location
        location: crate::map::Location,
    },
    /// open the named floater
    OpenFloater(String),
    /// open a floater describing a parcel
    Parcel(crate::key::ParcelKey),
    /// open a search floater with matching results
    Search {
        /// search category
        category: crate::search::SearchCategory,
        /// search term
        search_term: String,
    },
    /// open an inventory share/IM window for agent
    ShareWithAvatar(crate::key::AgentKey),
    /// teleport to this location
    Teleport(crate::map::Location),
    /// start a private voice session with this avatar
    VoiceCallAvatar(crate::key::AgentKey),
    /// replace outfit with contents of folder specified by key (UUID)
    WearFolderByInventoryFolderKey(crate::key::InventoryFolderKey),
    /// replace outfit with contents of named library folder
    WearFolderByLibraryFolderName(String),
    /// open the world map with this destination selected
    WorldMap(crate::map::Location),
}

impl ViewerUri {
    /// this returns whether the given ViewerUri can only be called from internal
    /// browsers/chat/... or if external programs (like browsers) can use them too
    #[must_use]
    pub const fn internal_only(&self) -> bool {
        matches!(self, Self::Location(_) | Self::Login { .. })
    }
}

impl std::fmt::Display for ViewerUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Location(location) => {
                write!(
                    f,
                    "secondlife:///{}/{}/{}/{}",
                    percent_encoding::percent_encode(
                        location.region_name().as_ref().as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    location.x(),
                    location.y(),
                    location.z()
                )
            }
            Self::AgentAbout(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/about")
            }
            Self::AgentInspect(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/inspect")
            }
            Self::AgentInstantMessage(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/im")
            }
            Self::AgentOfferTeleport(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/offerteleport")
            }
            Self::AgentPay(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/pay")
            }
            Self::AgentRequestFriend(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/requestfriend")
            }
            Self::AgentMute(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/mute")
            }
            Self::AgentUnmute(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/unmute")
            }
            Self::AgentCompleteName(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/completename")
            }
            Self::AgentDisplayName(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/displayname")
            }
            Self::AgentUsername(agent_key) => {
                write!(f, "secondlife:///app/agent/{agent_key}/username")
            }
            Self::AppearanceShow => {
                write!(f, "secondlife:///app/appearance/show")
            }
            Self::BalanceRequest => {
                write!(f, "secondlife:///app/balance/request")
            }
            Self::Chat { channel, text } => {
                write!(
                    f,
                    "secondlife:///app/chat/{}/{}",
                    channel,
                    percent_encoding::percent_encode(
                        text.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    )
                )
            }
            Self::ClassifiedAbout(classified_key) => {
                write!(f, "secondlife:///app/classified/{classified_key}/about")
            }
            Self::EventAbout(event_key) => {
                write!(f, "secondlife:///app/event/{event_key}/about")
            }
            Self::ExperienceProfile(experience_key) => {
                write!(f, "secondlife:///app/experience/{experience_key}/profile")
            }
            Self::GroupAbout(group_key) => {
                write!(f, "secondlife:///app/group/{group_key}/about")
            }
            Self::GroupInspect(group_key) => {
                write!(f, "secondlife:///app/group/{group_key}/inspect")
            }
            Self::GroupCreate => {
                write!(f, "secondlife:///app/group/create")
            }
            Self::GroupListShow => {
                write!(f, "secondlife:///app/group/list/show")
            }
            Self::Help { help_query } => {
                if let Some(help_query) = help_query {
                    write!(
                        f,
                        "secondlife:///app/help/{}",
                        percent_encoding::percent_encode(
                            help_query.as_bytes(),
                            percent_encoding::NON_ALPHANUMERIC
                        )
                    )
                } else {
                    write!(f, "secondlife:///app/help")
                }
            }
            Self::InventorySelect(inventory_key) => {
                write!(f, "secondlife:///app/inventory/{inventory_key}/select")
            }
            Self::InventoryShow => {
                write!(f, "secondlife:///app/inventory/show")
            }
            Self::KeyBindingMovementWalkTo => {
                write!(f, "secondlife:///app/keybinding/walk_to")
            }
            Self::KeyBindingMovementTeleportTo => {
                write!(f, "secondlife:///app/keybinding/teleport_to")
            }
            Self::KeyBindingMovementPushForward => {
                write!(f, "secondlife:///app/keybinding/push_forward")
            }
            Self::KeyBindingMovementPushBackward => {
                write!(f, "secondlife:///app/keybinding/push_backward")
            }
            Self::KeyBindingMovementTurnLeft => {
                write!(f, "secondlife:///app/keybinding/turn_left")
            }
            Self::KeyBindingMovementTurnRight => {
                write!(f, "secondlife:///app/keybinding/turn_right")
            }
            Self::KeyBindingMovementSlideLeft => {
                write!(f, "secondlife:///app/keybinding/slide_left")
            }
            Self::KeyBindingMovementSlideRight => {
                write!(f, "secondlife:///app/keybinding/slide_right")
            }
            Self::KeyBindingMovementJump => {
                write!(f, "secondlife:///app/keybinding/jump")
            }
            Self::KeyBindingMovementPushDown => {
                write!(f, "secondlife:///app/keybinding/push_down")
            }
            Self::KeyBindingMovementRunForward => {
                write!(f, "secondlife:///app/keybinding/run_forward")
            }
            Self::KeyBindingMovementRunBackward => {
                write!(f, "secondlife:///app/keybinding/run_backward")
            }
            Self::KeyBindingMovementRunLeft => {
                write!(f, "secondlife:///app/keybinding/run_left")
            }
            Self::KeyBindingMovementRunRight => {
                write!(f, "secondlife:///app/keybinding/run_right")
            }
            Self::KeyBindingMovementToggleRun => {
                write!(f, "secondlife:///app/keybinding/toggle_run")
            }
            Self::KeyBindingMovementToggleFly => {
                write!(f, "secondlife:///app/keybinding/toggle_fly")
            }
            Self::KeyBindingMovementToggleSit => {
                write!(f, "secondlife:///app/keybinding/toggle_sit")
            }
            Self::KeyBindingMovementStopMoving => {
                write!(f, "secondlife:///app/keybinding/stop_moving")
            }
            Self::KeyBindingCameraLookUp => {
                write!(f, "secondlife:///app/keybinding/look_up")
            }
            Self::KeyBindingCameraLookDown => {
                write!(f, "secondlife:///app/keybinding/look_down")
            }
            Self::KeyBindingCameraMoveForward => {
                write!(f, "secondlife:///app/keybinding/move_forward")
            }
            Self::KeyBindingCameraMoveBackward => {
                write!(f, "secondlife:///app/keybinding/move_backward")
            }
            Self::KeyBindingCameraMoveForwardFast => {
                write!(f, "secondlife:///app/keybinding/move_forward_fast")
            }
            Self::KeyBindingCameraMoveBackwardFast => {
                write!(f, "secondlife:///app/keybinding/move_backward_fast")
            }
            Self::KeyBindingCameraSpinOver => {
                write!(f, "secondlife:///app/keybinding/spin_over")
            }
            Self::KeyBindingCameraSpinUnder => {
                write!(f, "secondlife:///app/keybinding/spin_under")
            }
            Self::KeyBindingCameraPanUp => {
                write!(f, "secondlife:///app/keybinding/pan_up")
            }
            Self::KeyBindingCameraPanDown => {
                write!(f, "secondlife:///app/keybinding/pan_down")
            }
            Self::KeyBindingCameraPanLeft => {
                write!(f, "secondlife:///app/keybinding/pan_left")
            }
            Self::KeyBindingCameraPanRight => {
                write!(f, "secondlife:///app/keybinding/pan_right")
            }
            Self::KeyBindingCameraPanIn => {
                write!(f, "secondlife:///app/keybinding/pan_in")
            }
            Self::KeyBindingCameraPanOut => {
                write!(f, "secondlife:///app/keybinding/pan_out")
            }
            Self::KeyBindingCameraSpinAroundCounterClockwise => {
                write!(f, "secondlife:///app/keybinding/spin_around_ccw")
            }
            Self::KeyBindingCameraSpinAroundClockwise => {
                write!(f, "secondlife:///app/keybinding/spin_around_cw")
            }
            Self::KeyBindingCameraMoveForwardSitting => {
                write!(f, "secondlife:///app/keybinding/move_forward_sitting")
            }
            Self::KeyBindingCameraMoveBackwardSitting => {
                write!(f, "secondlife:///app/keybinding/move_backward_sitting")
            }
            Self::KeyBindingCameraSpinOverSitting => {
                write!(f, "secondlife:///app/keybinding/spin_over_sitting")
            }
            Self::KeyBindingCameraSpinUnderSitting => {
                write!(f, "secondlife:///app/keybinding/spin_under_sitting")
            }
            Self::KeyBindingCameraSpinAroundCounterClockwiseSitting => {
                write!(f, "secondlife:///app/keybinding/spin_around_ccw_sitting")
            }
            Self::KeyBindingCameraSpinAroundClockwiseSitting => {
                write!(f, "secondlife:///app/keybinding/spin_around_cw_sitting")
            }
            Self::KeyBindingEditingAvatarSpinCounterClockwise => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_ccw")
            }
            Self::KeyBindingEditingAvatarSpinClockwise => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_cw")
            }
            Self::KeyBindingEditingAvatarSpinOver => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_over")
            }
            Self::KeyBindingEditingAvatarSpinUnder => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_under")
            }
            Self::KeyBindingEditingAvatarMoveForward => {
                write!(f, "secondlife:///app/keybinding/avatar_move_forward")
            }
            Self::KeyBindingEditingAvatarMoveBackward => {
                write!(f, "secondlife:///app/keybinding/avatar_move_backward")
            }
            Self::KeyBindingSoundAndMediaTogglePauseMedia => {
                write!(f, "secondlife:///app/keybinding/toggle_pause_media")
            }
            Self::KeyBindingSoundAndMediaToggleEnableMedia => {
                write!(f, "secondlife:///app/keybinding/toggle_enable_media")
            }
            Self::KeyBindingSoundAndMediaVoiceFollowKey => {
                write!(f, "secondlife:///app/keybinding/voice_follow_key")
            }
            Self::KeyBindingSoundAndMediaToggleVoice => {
                write!(f, "secondlife:///app/keybinding/toggle_voice")
            }
            Self::KeyBindingStartChat => {
                write!(f, "secondlife:///app/keybinding/start_chat")
            }
            Self::KeyBindingStartGesture => {
                write!(f, "secondlife:///app/keybinding/start_gesture")
            }
            Self::KeyBindingScriptTriggerLButton(script_trigger_mode) => {
                write!(
                    f,
                    "secondlife:///app/keybinding/script_trigger_lbutton?mode={script_trigger_mode}"
                )
            }
            Self::Login {
                first_name,
                last_name,
                session,
                login_location,
            } => {
                write!(
                    f,
                    "secondlife::///app/login?first={}&last={}&session={}{}",
                    percent_encoding::percent_encode(
                        first_name.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    percent_encoding::percent_encode(
                        last_name.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    session,
                    if let Some(login_location) = login_location {
                        format!(
                            "&location={}",
                            percent_encoding::percent_encode(
                                login_location.as_bytes(),
                                percent_encoding::NON_ALPHANUMERIC
                            ),
                        )
                    } else {
                        "".to_string()
                    },
                )
            }
            Self::MapTrackAvatar(friend_key) => {
                write!(f, "secondlife:///app/maptrackavatar/{friend_key}")
            }
            Self::ObjectInstantMessage {
                object_key,
                object_name,
                owner,
                location,
            } => {
                write!(
                    f,
                    "secondlife::///app/objectim/{}/?object_name={}&{}&slurl={}/{}/{}/{}",
                    object_key,
                    percent_encoding::percent_encode(
                        object_name.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    match owner {
                        crate::key::OwnerKey::Agent(agent_key) => {
                            format!("owner={agent_key}")
                        }
                        crate::key::OwnerKey::Group(group_key) => {
                            format!("owner={group_key}?groupowned=true")
                        }
                    },
                    percent_encoding::percent_encode(
                        location.region_name.as_ref().as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    location.x,
                    location.y,
                    location.z,
                )
            }
            Self::OpenFloater(floater_name) => {
                write!(
                    f,
                    "secondlife:///app/openfloater/{}",
                    percent_encoding::percent_encode(
                        floater_name.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    )
                )
            }
            Self::Parcel(parcel_key) => {
                write!(f, "secondlife:///app/parcel/{parcel_key}/about")
            }
            Self::Search {
                category,
                search_term,
            } => {
                write!(
                    f,
                    "secondlife:///app/search/{}/{}",
                    category,
                    percent_encoding::percent_encode(
                        search_term.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    )
                )
            }
            Self::ShareWithAvatar(agent_key) => {
                write!(f, "secondlife::///app/sharewithavatar/{agent_key}")
            }
            Self::Teleport(location) => {
                write!(
                    f,
                    "secondlife:///teleport/{}/{}/{}/{}",
                    percent_encoding::percent_encode(
                        location.region_name().as_ref().as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    location.x(),
                    location.y(),
                    location.z()
                )
            }
            Self::VoiceCallAvatar(agent_key) => {
                write!(f, "secondlife:///app/voicecallavatar/{agent_key}")
            }
            Self::WearFolderByInventoryFolderKey(inventory_folder_key) => {
                write!(
                    f,
                    "secondlife:///app/wear_folder?folder_id={inventory_folder_key}"
                )
            }
            Self::WearFolderByLibraryFolderName(library_folder_name) => {
                write!(
                    f,
                    "secondlife:///app/wear_folder?folder_name={}",
                    percent_encoding::percent_encode(
                        library_folder_name.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                )
            }
            Self::WorldMap(location) => {
                write!(
                    f,
                    "secondlife:///app/worldmap/{}/{}/{}/{}",
                    percent_encoding::percent_encode(
                        location.region_name().as_ref().as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    location.x(),
                    location.y(),
                    location.z()
                )
            }
        }
    }
}

// TODO: FromStr instance

/// parse a viewer app agent URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_agent_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/agent/")
        .ignore_then(
            crate::key::agent_key_parser()
                .then_ignore(just("/about"))
                .map(ViewerUri::AgentAbout)
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/inspect"))
                    .map(ViewerUri::AgentInspect))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/im"))
                    .map(ViewerUri::AgentInstantMessage))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/offerteleport"))
                    .map(ViewerUri::AgentOfferTeleport))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/pay"))
                    .map(ViewerUri::AgentPay))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/requestfriend"))
                    .map(ViewerUri::AgentRequestFriend))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/mute"))
                    .map(ViewerUri::AgentMute))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/unmute"))
                    .map(ViewerUri::AgentUnmute))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/completename"))
                    .map(ViewerUri::AgentCompleteName))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/displayname"))
                    .map(ViewerUri::AgentDisplayName))
                .or(crate::key::agent_key_parser()
                    .then_ignore(just("/username"))
                    .map(ViewerUri::AgentUsername)),
        )
        .labelled("viewer app agent URI")
}

/// parse a viewer app appearance URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_appearance_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/appearance/show")
        .to(ViewerUri::AppearanceShow)
        .labelled("viewer app appearance URI")
}

/// parse a viewer app balance URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_balance_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/balance/request")
        .to(ViewerUri::BalanceRequest)
        .labelled("viewer app balance URI")
}

/// parse a viewer app chat URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_chat_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/chat/")
        .ignore_then(
            crate::chat::chat_channel_parser()
                .then_ignore(just('/'))
                .then(url_text_component_parser()),
        )
        .map(|(channel, text)| ViewerUri::Chat {
            channel,
            text: text.to_string(),
        })
        .labelled("viewer app chat URI")
}

/// parse a viewer app classified URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_classified_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/classified/")
        .ignore_then(
            crate::key::classified_key_parser()
                .then_ignore(just("/about"))
                .map(ViewerUri::ClassifiedAbout),
        )
        .labelled("viewer app classified URI")
}

/// parse a viewer app event URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_event_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/event/")
        .ignore_then(
            crate::key::event_key_parser()
                .then_ignore(just("/about"))
                .map(ViewerUri::EventAbout),
        )
        .labelled("viewer app event URI")
}

/// parse a viewer app experience URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_experience_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/experience/")
        .ignore_then(
            crate::key::experience_key_parser()
                .then_ignore(just("/profile"))
                .map(ViewerUri::ExperienceProfile),
        )
        .labelled("viewer app experience URI")
}

/// parse a viewer app group URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_group_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/group/")
        .ignore_then(
            crate::key::group_key_parser()
                .then_ignore(just("/about"))
                .map(ViewerUri::GroupAbout)
                .or(crate::key::group_key_parser()
                    .then_ignore(just("/inspect"))
                    .map(ViewerUri::GroupInspect))
                .or(just("create").to(ViewerUri::GroupCreate))
                .or(just("list/show").to(ViewerUri::GroupListShow)),
        )
        .labelled("viewer app group URI")
}

/// parse a viewer app help URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_help_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/help/")
        .ignore_then(just('/').ignore_then(url_text_component_parser()).or_not())
        .map(|help_query| ViewerUri::Help { help_query })
        .labelled("viewer app help URI")
}

/// parse a viewer app inventory URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_inventory_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/inventory/")
        .ignore_then(
            crate::key::inventory_key_parser()
                .then_ignore(just("/select"))
                .map(ViewerUri::InventorySelect)
                .or(just("/show").to(ViewerUri::InventoryShow)),
        )
        .labelled("viewer app inventory URI")
}

/// parse a viewer app keybinding URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_keybinding_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/keybinding/")
        .ignore_then(
            url_text_component_parser()
                .try_map(|s, span| match s.deref() {
                    "walk_to" => Ok(ViewerUri::KeyBindingMovementWalkTo),
                    "teleport_to" => Ok(ViewerUri::KeyBindingMovementTeleportTo),
                    "push_forward" => Ok(ViewerUri::KeyBindingMovementPushForward),
                    "push_backward" => Ok(ViewerUri::KeyBindingMovementPushBackward),
                    "turn_left" => Ok(ViewerUri::KeyBindingMovementTurnLeft),
                    "turn_right" => Ok(ViewerUri::KeyBindingMovementTurnRight),
                    "slide_left" => Ok(ViewerUri::KeyBindingMovementSlideLeft),
                    "slide_right" => Ok(ViewerUri::KeyBindingMovementSlideRight),
                    "jump" => Ok(ViewerUri::KeyBindingMovementJump),
                    "push_down" => Ok(ViewerUri::KeyBindingMovementPushDown),
                    "run_forward" => Ok(ViewerUri::KeyBindingMovementRunForward),
                    "run_backward" => Ok(ViewerUri::KeyBindingMovementRunBackward),
                    "run_left" => Ok(ViewerUri::KeyBindingMovementRunLeft),
                    "run_right" => Ok(ViewerUri::KeyBindingMovementRunRight),
                    "toggle_run" => Ok(ViewerUri::KeyBindingMovementToggleRun),
                    "toggle_fly" => Ok(ViewerUri::KeyBindingMovementToggleFly),
                    "toggle_sit" => Ok(ViewerUri::KeyBindingMovementToggleSit),
                    "stop_moving" => Ok(ViewerUri::KeyBindingMovementStopMoving),
                    "look_up" => Ok(ViewerUri::KeyBindingCameraLookUp),
                    "look_down" => Ok(ViewerUri::KeyBindingCameraLookDown),
                    "move_forward_fast" => Ok(ViewerUri::KeyBindingCameraMoveForwardFast),
                    "move_backward_fast" => Ok(ViewerUri::KeyBindingCameraMoveBackwardFast),
                    "move_forward_sitting" => Ok(ViewerUri::KeyBindingCameraMoveForwardSitting),
                    "move_backward_sittingk" => Ok(ViewerUri::KeyBindingCameraMoveBackwardSitting),
                    "move_forward" => Ok(ViewerUri::KeyBindingCameraMoveForward),
                    "move_backward" => Ok(ViewerUri::KeyBindingCameraMoveBackward),
                    "spin_over_sitting" => Ok(ViewerUri::KeyBindingCameraSpinOverSitting),
                    "spin_under_sitting" => Ok(ViewerUri::KeyBindingCameraSpinUnderSitting),
                    "spin_over" => Ok(ViewerUri::KeyBindingCameraSpinOver),
                    "spin_under" => Ok(ViewerUri::KeyBindingCameraSpinUnder),
                    "pan_up" => Ok(ViewerUri::KeyBindingCameraPanUp),
                    "pan_down" => Ok(ViewerUri::KeyBindingCameraPanDown),
                    "pan_left" => Ok(ViewerUri::KeyBindingCameraPanLeft),
                    "pan_right" => Ok(ViewerUri::KeyBindingCameraPanRight),
                    "pan_in" => Ok(ViewerUri::KeyBindingCameraPanIn),
                    "pan_out" => Ok(ViewerUri::KeyBindingCameraPanOut),
                    "spin_around_ccw_sitting" => {
                        Ok(ViewerUri::KeyBindingCameraSpinAroundCounterClockwiseSitting)
                    }
                    "spin_around_cw_sitting" => {
                        Ok(ViewerUri::KeyBindingCameraSpinAroundClockwiseSitting)
                    }
                    "spin_around_ccw" => Ok(ViewerUri::KeyBindingCameraSpinAroundCounterClockwise),
                    "spin_around_cw" => Ok(ViewerUri::KeyBindingCameraSpinAroundClockwise),
                    "edit_avatar_spin_ccw" => {
                        Ok(ViewerUri::KeyBindingEditingAvatarSpinCounterClockwise)
                    }
                    "edit_avatar_spin_cw" => Ok(ViewerUri::KeyBindingEditingAvatarSpinClockwise),
                    "edit_avatar_spin_over" => Ok(ViewerUri::KeyBindingEditingAvatarSpinOver),
                    "edit_avatar_spin_under" => Ok(ViewerUri::KeyBindingEditingAvatarSpinUnder),
                    "edit_avatar_move_forward" => Ok(ViewerUri::KeyBindingEditingAvatarMoveForward),
                    "edit_avatar_move_backward" => {
                        Ok(ViewerUri::KeyBindingEditingAvatarMoveBackward)
                    }
                    "toggle_pause_media" => Ok(ViewerUri::KeyBindingSoundAndMediaTogglePauseMedia),
                    "toggle_enable_media" => {
                        Ok(ViewerUri::KeyBindingSoundAndMediaToggleEnableMedia)
                    }
                    "voice_follow_key" => Ok(ViewerUri::KeyBindingSoundAndMediaVoiceFollowKey),
                    "toggle_voice" => Ok(ViewerUri::KeyBindingSoundAndMediaToggleVoice),
                    "start_chat" => Ok(ViewerUri::KeyBindingStartChat),
                    "start_gesture" => Ok(ViewerUri::KeyBindingStartGesture),
                    _ => Err(chumsky::error::Rich::custom(
                        span,
                        format!("Not a valid keybinding: {s}"),
                    )),
                })
                .or(just("/script_trigger_lbutton")
                    .ignore_then(script_trigger_mode_parser())
                    .map(ViewerUri::KeyBindingScriptTriggerLButton)),
        )
        .labelled("viewer app keybinding URI")
}

/// parse a viewer app login URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_login_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/login?first=")
        .ignore_then(url_text_component_parser())
        .then(just("?last=").ignore_then(url_text_component_parser()))
        .then(just("?session=").ignore_then(url_text_component_parser()))
        .then(
            just("?location=")
                .ignore_then(url_text_component_parser())
                .or_not(),
        )
        .map(
            |(((first_name, last_name), session), login_location)| ViewerUri::Login {
                first_name,
                last_name,
                session,
                login_location,
            },
        )
        .labelled("viewer app login URI")
}

/// parse a viewer app maptrackavatar URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_maptrackavatar_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/maptrackavatar/")
        .ignore_then(crate::key::friend_key_parser().map(ViewerUri::MapTrackAvatar))
        .labelled("viewer app maptrackavatar URI")
}

/// parse a viewer app objectim URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_objectim_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/objectim/")
        .ignore_then(crate::key::object_key_parser())
        .then_ignore(just('/').or_not())
        .then(just("?name=").ignore_then(url_text_component_parser()))
        .then(
            just("&owner=")
                .ignore_then(crate::key::group_key_parser())
                .then_ignore(just("&groupowned=true"))
                .map(crate::key::OwnerKey::Group)
                .or(just("&owner=")
                    .ignore_then(crate::key::agent_key_parser())
                    .map(crate::key::OwnerKey::Agent)),
        )
        .then(just("&slurl=").ignore_then(crate::map::url_encoded_location_parser()))
        .map(
            |(((object_key, object_name), owner), location)| ViewerUri::ObjectInstantMessage {
                object_key,
                object_name,
                owner,
                location,
            },
        )
        .labelled("viewer app objectim URI")
}

/// parse a viewer app openfloater URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_openfloater_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/openfloater/")
        .ignore_then(url_text_component_parser().map(ViewerUri::OpenFloater))
        .labelled("viewer app openfloater URI")
}

/// parse a viewer app parcel URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_parcel_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/parcel/")
        .ignore_then(crate::key::parcel_key_parser().map(ViewerUri::Parcel))
        .labelled("viewer app parcel URI")
}

/// parse a viewer app search URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_search_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/search/")
        .ignore_then(crate::search::search_category_parser())
        .then_ignore(just('/'))
        .then(url_text_component_parser())
        .map(|(category, search_term)| ViewerUri::Search {
            category,
            search_term,
        })
        .labelled("viewer app search URI")
}

/// parse a viewer app sharewithavatar URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_sharewithavatar_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/sharewithavatar/")
        .ignore_then(crate::key::agent_key_parser().map(ViewerUri::ShareWithAvatar))
        .labelled("viewer app sharewithavatar URI")
}
/// parse a viewer app teleport URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_teleport_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/teleport/")
        .ignore_then(crate::map::url_location_parser().map(ViewerUri::Teleport))
        .labelled("viewer app teleport URI")
}

/// parse a viewer app voicecallavatar URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_voicecallavatar_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/voicecallavatar/")
        .ignore_then(crate::key::agent_key_parser().map(ViewerUri::VoiceCallAvatar))
        .labelled("viewer app voicecallavatar URI")
}

/// parse a viewer app wear_folder URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_wear_folder_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/wear_folder")
        .ignore_then(
            just("?folder_id=")
                .ignore_then(crate::key::inventory_folder_key_parser())
                .map(ViewerUri::WearFolderByInventoryFolderKey)
                .or(just("?folder_name=")
                    .ignore_then(url_text_component_parser())
                    .map(ViewerUri::WearFolderByLibraryFolderName)),
        )
        .labelled("viewer app wear_folder URI")
}

/// parse a viewer app worldmap URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_worldmap_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///app/worldmap/")
        .ignore_then(crate::map::url_location_parser().map(ViewerUri::WorldMap))
        .labelled("viewer app worldmap URI")
}

/// parse a viewer app URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_app_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    viewer_app_agent_uri_parser()
        .or(viewer_app_appearance_uri_parser())
        .or(viewer_app_balance_uri_parser())
        .or(viewer_app_chat_uri_parser())
        .or(viewer_app_classified_uri_parser())
        .or(viewer_app_event_uri_parser())
        .or(viewer_app_experience_uri_parser())
        .or(viewer_app_group_uri_parser())
        .or(viewer_app_help_uri_parser())
        .or(viewer_app_inventory_uri_parser())
        .or(viewer_app_keybinding_uri_parser())
        .or(viewer_app_login_uri_parser())
        .or(viewer_app_maptrackavatar_uri_parser())
        .or(viewer_app_objectim_uri_parser())
        .or(viewer_app_openfloater_uri_parser())
        .or(viewer_app_parcel_uri_parser())
        .or(viewer_app_search_uri_parser())
        .or(viewer_app_sharewithavatar_uri_parser())
        .or(viewer_app_teleport_uri_parser())
        .or(viewer_app_voicecallavatar_uri_parser())
        .or(viewer_app_wear_folder_uri_parser())
        .or(viewer_app_worldmap_uri_parser())
        .labelled("viewer app URI")
}

/// parse a viewer location URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn viewer_location_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    just("secondlife:///")
        .ignore_then(crate::map::url_location_parser())
        .map(ViewerUri::Location)
        .labelled("viewer location URI")
}

/// parse a viewer URI
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
#[expect(
    clippy::module_name_repetitions,
    reason = "the parse is used outside this module"
)]
pub fn viewer_uri_parser<'src>()
-> impl Parser<'src, &'src str, ViewerUri, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    viewer_app_uri_parser()
        .or(viewer_location_uri_parser())
        .labelled("viewer URI")
}
