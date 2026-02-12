//! Second Life key (UUID) related types

use uuid::{Uuid, uuid};

#[cfg(feature = "chumsky")]
use chumsky::{
    IterParser as _, Parser,
    prelude::{just, one_of},
};

/// parse a UUID
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn uuid_parser<'src>()
-> impl Parser<'src, &'src str, uuid::Uuid, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    one_of("0123456789abcdef")
        .repeated()
        .exactly(8)
        .collect::<String>()
        .then_ignore(just('-'))
        .then(
            one_of("0123456789abcdef")
                .repeated()
                .exactly(4)
                .collect::<String>(),
        )
        .then_ignore(just('-'))
        .then(
            one_of("0123456789abcdef")
                .repeated()
                .exactly(4)
                .collect::<String>(),
        )
        .then_ignore(just('-'))
        .then(
            one_of("0123456789abcdef")
                .repeated()
                .exactly(4)
                .collect::<String>(),
        )
        .then_ignore(just('-'))
        .then(
            one_of("0123456789abcdef")
                .repeated()
                .exactly(12)
                .collect::<String>(),
        )
        .try_map(|((((a, b), c), d), e), span: chumsky::span::SimpleSpan| {
            uuid::Uuid::parse_str(&format!("{a}-{b}-{c}-{d}-{e}"))
                .map_err(|e| chumsky::error::Rich::custom(span, format!("{e:?}")))
        })
}

/// represents a general Second Life key without any knowledge about the type
/// of entity this represents
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key(pub Uuid);

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// parse a Key
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
#[expect(
    clippy::module_name_repetitions,
    reason = "the parser is going to be used outside this module"
)]
pub fn key_parser<'src>()
-> impl Parser<'src, &'src str, Key, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    uuid_parser().map(Key)
}

/// the null key used by Second Life in many places to represent the absence
/// of a key value
pub const NULL_KEY: Key = Key(uuid!("00000000-0000-0000-0000-000000000000"));

/// the key used by the Second Life system to send combat logs to the COMBAT_CHANNEL
pub const COMBAT_LOG_ID: Key = Key(uuid!("45e0fcfa-2268-4490-a51c-3e51bdfe80d1"));

/// represents a Second Life key for an agent (avatar)
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct AgentKey(pub Key);

impl std::fmt::Display for AgentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<AgentKey> for Key {
    fn from(val: AgentKey) -> Self {
        val.0
    }
}

/// parse an AgentKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn agent_key_parser<'src>()
-> impl Parser<'src, &'src str, AgentKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(AgentKey)
}

/// parse a viewer URI that is either an /about or /inspect URL for an avatar
/// and return the AgentKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn app_agent_uri_as_agent_key_parser<'src>()
-> impl Parser<'src, &'src str, AgentKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    crate::viewer_uri::viewer_app_agent_uri_parser().try_map(|uri, span| match uri {
        crate::viewer_uri::ViewerUri::AgentAbout(agent_key)
        | crate::viewer_uri::ViewerUri::AgentInspect(agent_key) => Ok(agent_key),
        _ => Err(chumsky::error::Rich::custom(
            span,
            "Unexpected type of Agent viewer URI",
        )),
    })
}

/// represents a Second Life key for a classified ad
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct ClassifiedKey(pub Key);

impl std::fmt::Display for ClassifiedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ClassifiedKey> for Key {
    fn from(val: ClassifiedKey) -> Self {
        val.0
    }
}

/// parse a ClassifiedKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn classified_key_parser<'src>()
-> impl Parser<'src, &'src str, ClassifiedKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    key_parser().map(ClassifiedKey)
}

/// represents a Second Life key for an event
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct EventKey(pub Key);

impl std::fmt::Display for EventKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<EventKey> for Key {
    fn from(val: EventKey) -> Self {
        val.0
    }
}

/// parse an EventKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn event_key_parser<'src>()
-> impl Parser<'src, &'src str, EventKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(EventKey)
}

/// represents a Second Life key for an experience
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct ExperienceKey(pub Key);

impl std::fmt::Display for ExperienceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ExperienceKey> for Key {
    fn from(val: ExperienceKey) -> Self {
        val.0
    }
}

/// parse an ExperienceKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn experience_key_parser<'src>()
-> impl Parser<'src, &'src str, ExperienceKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    key_parser().map(ExperienceKey)
}

/// represents a Second Life key for an agent who is a friend
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct FriendKey(pub Key);

impl std::fmt::Display for FriendKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<FriendKey> for Key {
    fn from(val: FriendKey) -> Self {
        val.0
    }
}

impl From<FriendKey> for AgentKey {
    fn from(val: FriendKey) -> Self {
        Self(val.0)
    }
}

/// parse a FriendKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn friend_key_parser<'src>()
-> impl Parser<'src, &'src str, FriendKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(FriendKey)
}

/// represents a Second Life key for a group
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct GroupKey(pub Key);

impl std::fmt::Display for GroupKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<GroupKey> for Key {
    fn from(val: GroupKey) -> Self {
        val.0
    }
}

/// parse a GroupKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn group_key_parser<'src>()
-> impl Parser<'src, &'src str, GroupKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(GroupKey)
}

/// parse a viewer URI that is either an /about or /inspect URL for a group
/// and return the GroupKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn app_group_uri_as_group_key_parser<'src>()
-> impl Parser<'src, &'src str, GroupKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    crate::viewer_uri::viewer_app_group_uri_parser().try_map(|uri, span| match uri {
        crate::viewer_uri::ViewerUri::GroupAbout(group_key)
        | crate::viewer_uri::ViewerUri::GroupInspect(group_key) => Ok(group_key),
        _ => Err(chumsky::error::Rich::custom(
            span,
            "Unexpected type of group viewer URI",
        )),
    })
}

/// represents a Second Life key for an inventory item
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct InventoryKey(pub Key);

impl std::fmt::Display for InventoryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<InventoryKey> for Key {
    fn from(val: InventoryKey) -> Self {
        val.0
    }
}

/// parse an InventoryKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn inventory_key_parser<'src>()
-> impl Parser<'src, &'src str, InventoryKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    key_parser().map(InventoryKey)
}

/// represents a Second Life key for an object
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct ObjectKey(pub Key);

impl std::fmt::Display for ObjectKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ObjectKey> for Key {
    fn from(val: ObjectKey) -> Self {
        val.0
    }
}

/// parse an ObjectKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn object_key_parser<'src>()
-> impl Parser<'src, &'src str, ObjectKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(ObjectKey)
}

/// represents a Second Life key for a parcel
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct ParcelKey(pub Key);

impl std::fmt::Display for ParcelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ParcelKey> for Key {
    fn from(val: ParcelKey) -> Self {
        val.0
    }
}

/// parse a ParcelKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn parcel_key_parser<'src>()
-> impl Parser<'src, &'src str, ParcelKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(ParcelKey)
}

/// represents a Second Life key for a texture
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct TextureKey(pub Key);

impl std::fmt::Display for TextureKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TextureKey> for Key {
    fn from(val: TextureKey) -> Self {
        val.0
    }
}

/// parse a TextureKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn texture_key_parser<'src>()
-> impl Parser<'src, &'src str, TextureKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(TextureKey)
}

/// represents a Second Life key for an inventory folder
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct InventoryFolderKey(pub Key);

impl std::fmt::Display for InventoryFolderKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<InventoryFolderKey> for Key {
    fn from(val: InventoryFolderKey) -> Self {
        val.0
    }
}

/// parse an InventoryFolderKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn inventory_folder_key_parser<'src>() -> impl Parser<
    'src,
    &'src str,
    InventoryFolderKey,
    chumsky::extra::Err<chumsky::error::Rich<'src, char>>,
> {
    key_parser().map(InventoryFolderKey)
}

/// represents s Second Life key for an owner (e.g. of an object)
#[derive(Debug, Clone, PartialEq, Eq, strum::EnumIs)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub enum OwnerKey {
    /// the owner is an agent
    Agent(AgentKey),
    /// the owner is a group
    Group(GroupKey),
}

impl std::fmt::Display for OwnerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(agent_key) => write!(f, "{agent_key}"),
            Self::Group(group_key) => write!(f, "{group_key}"),
        }
    }
}

/// error when the owner is a group while trying to convert an OwnerKey to an AgentKey
#[derive(Debug, Clone)]
pub struct OwnerIsGroupError(GroupKey);

impl std::fmt::Display for OwnerIsGroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "The owner is not an agent but the group {}", self.0)
    }
}

impl TryInto<AgentKey> for OwnerKey {
    type Error = OwnerIsGroupError;

    fn try_into(self) -> Result<AgentKey, Self::Error> {
        match self {
            Self::Agent(agent_key) => Ok(agent_key),
            Self::Group(group_key) => Err(OwnerIsGroupError(group_key)),
        }
    }
}

/// error when the owner is an agent while trying to convert an OwnerKey to a GroupKey
#[derive(Debug, Clone)]
pub struct OwnerIsAgentError(AgentKey);

impl std::fmt::Display for OwnerIsAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "The owner is not a group but the agent {}", self.0)
    }
}

impl TryInto<GroupKey> for OwnerKey {
    type Error = OwnerIsAgentError;

    fn try_into(self) -> Result<GroupKey, Self::Error> {
        match self {
            Self::Agent(agent_key) => Err(OwnerIsAgentError(agent_key)),
            Self::Group(group_key) => Ok(group_key),
        }
    }
}

impl From<OwnerKey> for Key {
    fn from(val: OwnerKey) -> Self {
        match val {
            OwnerKey::Agent(agent_key) => agent_key.into(),
            OwnerKey::Group(group_key) => group_key.into(),
        }
    }
}

/// parse a viewer URI that is either an /about or /inspect URL for an agent group
/// or for a group and return the OwnerKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn app_agent_or_group_uri_as_owner_key_parser<'src>()
-> impl Parser<'src, &'src str, OwnerKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    app_agent_uri_as_agent_key_parser()
        .map(OwnerKey::Agent)
        .or(app_group_uri_as_group_key_parser().map(OwnerKey::Group))
}
