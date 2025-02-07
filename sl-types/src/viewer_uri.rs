//! Viewer URI related types
//!
//! see https://wiki.secondlife.com/wiki/Viewer_URI_Name_Space

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
    // TODO: key bindings
    // TODO: login
    /// track a friend with the permission on the world map
    MapTrackAvatar(crate::key::FriendKey),
    // TODO: objectim
    // TODO: openfloater
    /// open a floater describing a parcel
    Parcel(crate::key::ParcelKey),
    // TODO: search
    // TODO: sharewithavatar
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
            ViewerUri::MapTrackAvatar(friend_key) => {
                write!(f, "secondlife:///app/maptrackavatar/{}", friend_key)
            }
            ViewerUri::Parcel(parcel_key) => {
                write!(f, "secondlife:///app/parcel/{}/about", parcel_key)
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
