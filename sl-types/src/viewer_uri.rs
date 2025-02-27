//! Viewer URI related types
//!
//! see https://wiki.secondlife.com/wiki/Viewer_URI_Name_Space

/// represents the various script trigger modes for the script_trigger_lbutton
/// key binding
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScriptTriggerMode {
    /// "first_person" or 0
    FirstPerson,
    /// "third_person" or 1
    ThirdPerson,
    /// "edit_avatar" or 2
    EditAvatar,
    /// "sitting" or 3
    Sitting,
}

impl std::fmt::Display for ScriptTriggerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptTriggerMode::FirstPerson => write!(f, "first_person"),
            ScriptTriggerMode::ThirdPerson => write!(f, "third_person"),
            ScriptTriggerMode::EditAvatar => write!(f, "edit_avatar"),
            ScriptTriggerMode::Sitting => write!(f, "sitting"),
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
            "first_person" => Ok(Self::FirstPerson),
            "0" => Ok(Self::FirstPerson),
            "third_person" => Ok(Self::ThirdPerson),
            "1" => Ok(Self::ThirdPerson),
            "edit_aatar" => Ok(Self::EditAvatar),
            "2" => Ok(Self::EditAvatar),
            "sitting" => Ok(Self::Sitting),
            "3" => Ok(Self::Sitting),
            _ => Err(ScriptTriggerModeParseError {
                value: s.to_owned(),
            }),
        }
    }
}

/// represents a Viewer URI
#[derive(Debug, Clone, PartialEq, Eq)]
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
    ExperienceAbout(crate::key::ExperienceKey),
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
    // TODO: login
    /// track a friend with the permission on the world map
    MapTrackAvatar(crate::key::FriendKey),
    /// display an info disalog for the object sending this message
    ObjectInstantMessage {
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
    // TODO: wear_folder
    /// open the world map with this destination selected
    WorldMap(crate::map::Location),
}

impl ViewerUri {
    /// this returns whether the given ViewerUri can only be called from internal
    /// browsers/chat/... or if external programs (like browsers) can use them too
    pub fn internal_only(&self) -> bool {
        match self {
            ViewerUri::Location(_) => false,
            _ => true,
        }
    }
}

impl std::fmt::Display for ViewerUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewerUri::Location(location) => {
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
            ViewerUri::AgentAbout(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/about", agent_key)
            }
            ViewerUri::AgentInspect(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/inspect", agent_key)
            }
            ViewerUri::AgentInstantMessage(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/im", agent_key)
            }
            ViewerUri::AgentOfferTeleport(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/offerteleport", agent_key)
            }
            ViewerUri::AgentPay(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/pay", agent_key)
            }
            ViewerUri::AgentRequestFriend(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/requestfriend", agent_key)
            }
            ViewerUri::AgentMute(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/mute", agent_key)
            }
            ViewerUri::AgentUnmute(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/unmute", agent_key)
            }
            ViewerUri::AgentCompleteName(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/completename", agent_key)
            }
            ViewerUri::AgentDisplayName(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/displayname", agent_key)
            }
            ViewerUri::AgentUsername(agent_key) => {
                write!(f, "secondlife:///app/agent/{}/username", agent_key)
            }
            ViewerUri::AppearanceShow => {
                write!(f, "secondlife:///app/appearance/show")
            }
            ViewerUri::BalanceRequest => {
                write!(f, "secondlife:///app/balance/request")
            }
            ViewerUri::Chat { channel, text } => {
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
            ViewerUri::ClassifiedAbout(classified_key) => {
                write!(f, "secondlife:///app/classified/{}/about", classified_key)
            }
            ViewerUri::EventAbout(event_key) => {
                write!(f, "secondlife:///app/event/{}/about", event_key)
            }
            ViewerUri::ExperienceAbout(experience_key) => {
                write!(f, "secondlife:///app/experience/{}/about", experience_key)
            }
            ViewerUri::GroupAbout(group_key) => {
                write!(f, "secondlife:///app/group/{}/about", group_key)
            }
            ViewerUri::GroupInspect(group_key) => {
                write!(f, "secondlife:///app/group/{}/inspect", group_key)
            }
            ViewerUri::GroupCreate => {
                write!(f, "secondlife:///app/group/create")
            }
            ViewerUri::GroupListShow => {
                write!(f, "secondlife:///app/group/list/show")
            }
            ViewerUri::Help { help_query } => {
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
            ViewerUri::InventorySelect(inventory_key) => {
                write!(f, "secondlife:///app/inventory/{}/select", inventory_key)
            }
            ViewerUri::InventoryShow => {
                write!(f, "secondlife:///app/inventory/show")
            }
            ViewerUri::KeyBindingMovementWalkTo => {
                write!(f, "secondlife:///app/keybinding/walk_to")
            }
            ViewerUri::KeyBindingMovementTeleportTo => {
                write!(f, "secondlife:///app/keybinding/teleport_to")
            }
            ViewerUri::KeyBindingMovementPushForward => {
                write!(f, "secondlife:///app/keybinding/push_forward")
            }
            ViewerUri::KeyBindingMovementPushBackward => {
                write!(f, "secondlife:///app/keybinding/push_backward")
            }
            ViewerUri::KeyBindingMovementTurnLeft => {
                write!(f, "secondlife:///app/keybinding/turn_left")
            }
            ViewerUri::KeyBindingMovementTurnRight => {
                write!(f, "secondlife:///app/keybinding/turn_right")
            }
            ViewerUri::KeyBindingMovementSlideLeft => {
                write!(f, "secondlife:///app/keybinding/slide_left")
            }
            ViewerUri::KeyBindingMovementSlideRight => {
                write!(f, "secondlife:///app/keybinding/slide_right")
            }
            ViewerUri::KeyBindingMovementJump => {
                write!(f, "secondlife:///app/keybinding/jump")
            }
            ViewerUri::KeyBindingMovementPushDown => {
                write!(f, "secondlife:///app/keybinding/push_down")
            }
            ViewerUri::KeyBindingMovementRunForward => {
                write!(f, "secondlife:///app/keybinding/run_forward")
            }
            ViewerUri::KeyBindingMovementRunBackward => {
                write!(f, "secondlife:///app/keybinding/run_backward")
            }
            ViewerUri::KeyBindingMovementRunLeft => {
                write!(f, "secondlife:///app/keybinding/run_left")
            }
            ViewerUri::KeyBindingMovementRunRight => {
                write!(f, "secondlife:///app/keybinding/run_right")
            }
            ViewerUri::KeyBindingMovementToggleRun => {
                write!(f, "secondlife:///app/keybinding/toggle_run")
            }
            ViewerUri::KeyBindingMovementToggleFly => {
                write!(f, "secondlife:///app/keybinding/toggle_fly")
            }
            ViewerUri::KeyBindingMovementToggleSit => {
                write!(f, "secondlife:///app/keybinding/toggle_sit")
            }
            ViewerUri::KeyBindingMovementStopMoving => {
                write!(f, "secondlife:///app/keybinding/stop_moving")
            }
            ViewerUri::KeyBindingCameraLookUp => {
                write!(f, "secondlife:///app/keybinding/look_up")
            }
            ViewerUri::KeyBindingCameraLookDown => {
                write!(f, "secondlife:///app/keybinding/look_down")
            }
            ViewerUri::KeyBindingCameraMoveForward => {
                write!(f, "secondlife:///app/keybinding/move_forward")
            }
            ViewerUri::KeyBindingCameraMoveBackward => {
                write!(f, "secondlife:///app/keybinding/move_backward")
            }
            ViewerUri::KeyBindingCameraMoveForwardFast => {
                write!(f, "secondlife:///app/keybinding/move_forward_fast")
            }
            ViewerUri::KeyBindingCameraMoveBackwardFast => {
                write!(f, "secondlife:///app/keybinding/move_backward_fast")
            }
            ViewerUri::KeyBindingCameraSpinOver => {
                write!(f, "secondlife:///app/keybinding/spin_over")
            }
            ViewerUri::KeyBindingCameraSpinUnder => {
                write!(f, "secondlife:///app/keybinding/spin_under")
            }
            ViewerUri::KeyBindingCameraPanUp => {
                write!(f, "secondlife:///app/keybinding/pan_up")
            }
            ViewerUri::KeyBindingCameraPanDown => {
                write!(f, "secondlife:///app/keybinding/pan_down")
            }
            ViewerUri::KeyBindingCameraPanLeft => {
                write!(f, "secondlife:///app/keybinding/pan_left")
            }
            ViewerUri::KeyBindingCameraPanRight => {
                write!(f, "secondlife:///app/keybinding/pan_right")
            }
            ViewerUri::KeyBindingCameraPanIn => {
                write!(f, "secondlife:///app/keybinding/pan_in")
            }
            ViewerUri::KeyBindingCameraPanOut => {
                write!(f, "secondlife:///app/keybinding/pan_out")
            }
            ViewerUri::KeyBindingCameraSpinAroundCounterClockwise => {
                write!(f, "secondlife:///app/keybinding/spin_around_ccw")
            }
            ViewerUri::KeyBindingCameraSpinAroundClockwise => {
                write!(f, "secondlife:///app/keybinding/spin_around_cw")
            }
            ViewerUri::KeyBindingCameraMoveForwardSitting => {
                write!(f, "secondlife:///app/keybinding/move_forward_sitting")
            }
            ViewerUri::KeyBindingCameraMoveBackwardSitting => {
                write!(f, "secondlife:///app/keybinding/move_backward_sitting")
            }
            ViewerUri::KeyBindingCameraSpinOverSitting => {
                write!(f, "secondlife:///app/keybinding/spin_over_sitting")
            }
            ViewerUri::KeyBindingCameraSpinUnderSitting => {
                write!(f, "secondlife:///app/keybinding/spin_under_sitting")
            }
            ViewerUri::KeyBindingCameraSpinAroundCounterClockwiseSitting => {
                write!(f, "secondlife:///app/keybinding/spin_around_ccw_sitting")
            }
            ViewerUri::KeyBindingCameraSpinAroundClockwiseSitting => {
                write!(f, "secondlife:///app/keybinding/spin_around_cw_sitting")
            }
            ViewerUri::KeyBindingEditingAvatarSpinCounterClockwise => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_ccw")
            }
            ViewerUri::KeyBindingEditingAvatarSpinClockwise => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_cw")
            }
            ViewerUri::KeyBindingEditingAvatarSpinOver => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_over")
            }
            ViewerUri::KeyBindingEditingAvatarSpinUnder => {
                write!(f, "secondlife:///app/keybinding/avatar_spin_under")
            }
            ViewerUri::KeyBindingEditingAvatarMoveForward => {
                write!(f, "secondlife:///app/keybinding/avatar_move_forward")
            }
            ViewerUri::KeyBindingEditingAvatarMoveBackward => {
                write!(f, "secondlife:///app/keybinding/avatar_move_backward")
            }
            ViewerUri::KeyBindingSoundAndMediaTogglePauseMedia => {
                write!(f, "secondlife:///app/keybinding/toggle_pause_media")
            }
            ViewerUri::KeyBindingSoundAndMediaToggleEnableMedia => {
                write!(f, "secondlife:///app/keybinding/toggle_enable_media")
            }
            ViewerUri::KeyBindingSoundAndMediaVoiceFollowKey => {
                write!(f, "secondlife:///app/keybinding/voice_follow_key")
            }
            ViewerUri::KeyBindingSoundAndMediaToggleVoice => {
                write!(f, "secondlife:///app/keybinding/toggle_voice")
            }
            ViewerUri::KeyBindingStartChat => {
                write!(f, "secondlife:///app/keybinding/start_chat")
            }
            ViewerUri::KeyBindingStartGesture => {
                write!(f, "secondlife:///app/keybinding/start_gesture")
            }
            ViewerUri::KeyBindingScriptTriggerLButton(script_trigger_mode) => {
                write!(
                    f,
                    "secondlife:///app/keybinding/script_trigger_lbutton?mode={}",
                    script_trigger_mode
                )
            }
            ViewerUri::MapTrackAvatar(friend_key) => {
                write!(f, "secondlife:///app/maptrackavatar/{}", friend_key)
            }
            ViewerUri::ObjectInstantMessage {
                object_name,
                owner,
                location,
            } => {
                write!(
                    f,
                    "secondlife::///app/objectim?object_name={}&{}&slurl={}/{}/{}/{}",
                    percent_encoding::percent_encode(
                        object_name.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    match owner {
                        crate::key::OwnerKey::Agent(agent_key) => {
                            format!("owner={}", agent_key)
                        }
                        crate::key::OwnerKey::Group(group_key) => {
                            format!("owner={}?groupowned=true", group_key)
                        }
                    },
                    percent_encoding::percent_encode(
                        location.region_name.clone().into_inner().as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    ),
                    location.x,
                    location.y,
                    location.z,
                )
            }
            ViewerUri::OpenFloater(floater_name) => {
                write!(
                    f,
                    "secondlife:///app/openfloater/{}",
                    percent_encoding::percent_encode(
                        floater_name.as_bytes(),
                        percent_encoding::NON_ALPHANUMERIC
                    )
                )
            }
            ViewerUri::Parcel(parcel_key) => {
                write!(f, "secondlife:///app/parcel/{}/about", parcel_key)
            }
            ViewerUri::Search {
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
            ViewerUri::ShareWithAvatar(agent_key) => {
                write!(f, "secondlife::///app/sharewithavatar/{}", agent_key)
            }
            ViewerUri::Teleport(location) => {
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
            ViewerUri::VoiceCallAvatar(agent_key) => {
                write!(f, "secondlife:///app/voicecallavatar/{}", agent_key)
            }
            ViewerUri::WorldMap(location) => {
                write!(
                    f,
                    "secondlife:///worldmap/{}/{}/{}/{}",
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
