//! Second Life key (UUID) related types

use uuid::{uuid, Uuid};

/// represents a general Second Life key without any knowledge about the type
/// of entity this represents
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key(pub Uuid);

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// the null key used by Second Life in many places to represent the absence
/// of a key value
pub const NULL_KEY: Key = Key(uuid!("00000000-0000-0000-0000-000000000000"));

/// the key used by the Second Life system to send combat logs to the COMBAT_CHANNEL
pub const COMBAT_LOG_ID: Key = Key(uuid!("45e0fcfa-2268-4490-a51c-3e51bdfe80d1"));

/// represents a Second Life key for an agent (avatar)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentKey(pub Key);

impl std::fmt::Display for AgentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for AgentKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for a classified ad
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedKey(pub Key);

impl std::fmt::Display for ClassifiedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for ClassifiedKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for an event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventKey(pub Key);

impl std::fmt::Display for EventKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for EventKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for an experience
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceKey(pub Key);

impl std::fmt::Display for ExperienceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for ExperienceKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for an agent who is a friend
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendKey(pub Key);

impl std::fmt::Display for FriendKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for FriendKey {
    fn into(self) -> Key {
        self.0
    }
}

impl Into<AgentKey> for FriendKey {
    fn into(self) -> AgentKey {
        AgentKey(self.0)
    }
}

/// represents a Second Life key for a group
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupKey(pub Key);

impl std::fmt::Display for GroupKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for GroupKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for an inventory item
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryKey(pub Key);

impl std::fmt::Display for InventoryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for InventoryKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for an object
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectKey(pub Key);

impl std::fmt::Display for ObjectKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for ObjectKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for a parcel
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParcelKey(pub Key);

impl std::fmt::Display for ParcelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for ParcelKey {
    fn into(self) -> Key {
        self.0
    }
}

/// represents a Second Life key for a texture
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureKey(pub Key);

impl std::fmt::Display for TextureKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Key> for TextureKey {
    fn into(self) -> Key {
        self.0
    }
}
