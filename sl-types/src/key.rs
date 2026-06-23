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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// represents a Second Life key for an experience
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIs)]
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

impl Key {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for Key {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AgentKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for AgentKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl ClassifiedKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for ClassifiedKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl ExperienceKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for ExperienceKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl FriendKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for FriendKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl GroupKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for GroupKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl InventoryKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for InventoryKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl ObjectKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for ObjectKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl ParcelKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for ParcelKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl TextureKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for TextureKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl InventoryFolderKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for InventoryFolderKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

impl OwnerKey {
    /// The wrapped raw UUID, regardless of whether the owner is an agent or a
    /// group.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        match self {
            Self::Agent(agent_key) => agent_key.0.0,
            Self::Group(group_key) => group_key.0.0,
        }
    }
}

/// represents a Second Life key for a *role* within a group (e.g. the "Owners"
/// role, or the nil-keyed default "Everyone" role), as distinct from the
/// group's own [`GroupKey`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct GroupRoleKey(pub Key);

impl std::fmt::Display for GroupRoleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<GroupRoleKey> for Key {
    fn from(val: GroupRoleKey) -> Self {
        val.0
    }
}

impl GroupRoleKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for GroupRoleKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

/// parse a GroupRoleKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn group_role_key_parser<'src>()
-> impl Parser<'src, &'src str, GroupRoleKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    key_parser().map(GroupRoleKey)
}

/// represents a Second Life key for a *mesh* asset — the UUID of a mesh asset,
/// as carried in the sculpt/mesh block of a prim whose shape comes from a mesh
/// rather than a sculpt texture. A mesh asset is emphatically not a
/// [`TextureKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct MeshKey(pub Key);

impl std::fmt::Display for MeshKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<MeshKey> for Key {
    fn from(val: MeshKey) -> Self {
        val.0
    }
}

impl MeshKey {
    /// The wrapped raw UUID.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0.0
    }
}

impl From<Uuid> for MeshKey {
    fn from(value: Uuid) -> Self {
        Self(Key(value))
    }
}

/// parse a MeshKey
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn mesh_key_parser<'src>()
-> impl Parser<'src, &'src str, MeshKey, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    key_parser().map(MeshKey)
}

/// a key that is either an **agent** ([`AgentKey`]) or an in-world **object**
/// ([`ObjectKey`]), as selected by a separate source-type discriminator (an
/// avatar or a prim can be the source of chat, sounds, effects, …)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIs)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub enum AgentOrObjectKey {
    /// the key is an agent
    Agent(AgentKey),
    /// the key is an in-world object
    Object(ObjectKey),
}

impl AgentOrObjectKey {
    /// The wrapped raw UUID, regardless of whether the key is an agent or an
    /// object.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        match self {
            Self::Agent(agent_key) => agent_key.0.0,
            Self::Object(object_key) => object_key.0.0,
        }
    }
}

impl std::fmt::Display for AgentOrObjectKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(agent_key) => write!(f, "{agent_key}"),
            Self::Object(object_key) => write!(f, "{object_key}"),
        }
    }
}

impl From<AgentOrObjectKey> for Key {
    fn from(val: AgentOrObjectKey) -> Self {
        match val {
            AgentOrObjectKey::Agent(agent_key) => agent_key.into(),
            AgentOrObjectKey::Object(object_key) => object_key.into(),
        }
    }
}

/// a key that is either an inventory **item** ([`InventoryKey`]) or a whole
/// inventory **folder/category** ([`InventoryFolderKey`]), as selected by a
/// separate asset-type discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIs)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub enum InventoryItemOrFolderKey {
    /// the key refers to a single inventory item
    Item(InventoryKey),
    /// the key refers to a whole inventory folder/category
    Folder(InventoryFolderKey),
}

impl InventoryItemOrFolderKey {
    /// The wrapped raw UUID, regardless of whether it refers to an item or a
    /// folder.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        match self {
            Self::Item(item_key) => item_key.0.0,
            Self::Folder(folder_key) => folder_key.0.0,
        }
    }
}

impl std::fmt::Display for InventoryItemOrFolderKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Item(item_key) => write!(f, "{item_key}"),
            Self::Folder(folder_key) => write!(f, "{folder_key}"),
        }
    }
}

impl From<InventoryItemOrFolderKey> for Key {
    fn from(val: InventoryItemOrFolderKey) -> Self {
        match val {
            InventoryItemOrFolderKey::Item(item_key) => item_key.into(),
            InventoryItemOrFolderKey::Folder(folder_key) => folder_key.into(),
        }
    }
}

/// the asset backing a prim's sculpt/mesh shape: either a sculpt **texture**
/// ([`TextureKey`]) or a **mesh** asset ([`MeshKey`]), as selected by the prim's
/// sculpt-type discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIs)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub enum SculptOrMeshKey {
    /// the prim is a sculpty: the key is a sculpt texture
    Sculpt(TextureKey),
    /// the prim is a mesh: the key is a mesh asset
    Mesh(MeshKey),
}

impl SculptOrMeshKey {
    /// The wrapped raw UUID, regardless of whether it is a sculpt texture or a
    /// mesh asset.
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        match self {
            Self::Sculpt(texture_key) => texture_key.0.0,
            Self::Mesh(mesh_key) => mesh_key.0.0,
        }
    }
}

impl std::fmt::Display for SculptOrMeshKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sculpt(texture_key) => write!(f, "{texture_key}"),
            Self::Mesh(mesh_key) => write!(f, "{mesh_key}"),
        }
    }
}

impl From<SculptOrMeshKey> for Key {
    fn from(val: SculptOrMeshKey) -> Self {
        match val {
            SculptOrMeshKey::Sculpt(texture_key) => texture_key.into(),
            SculptOrMeshKey::Mesh(mesh_key) => mesh_key.into(),
        }
    }
}
