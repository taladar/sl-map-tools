//! Friendship-related value types.

use crate::serde_helpers::impl_bitfield_serde;

/// The rights one party grants the other in a Second Life friendship: a
/// bitfield shared by the login `buddy-list`, `GrantUserRights`, and
/// `ChangeUserRights`. The flag values match the viewer's `RIGHTS_*`/`GRANT_*`
/// constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct FriendRights(pub i32);

impl FriendRights {
    /// The other party may see when this party is online (`GRANT_ONLINE_STATUS`).
    pub const CAN_SEE_ONLINE: i32 = 1 << 0;
    /// The other party may see this party's location on the world map
    /// (`GRANT_MAP_LOCATION`).
    pub const CAN_SEE_ON_MAP: i32 = 1 << 1;
    /// The other party may modify this party's objects (`GRANT_MODIFY_OBJECTS`).
    pub const CAN_MODIFY_OBJECTS: i32 = 1 << 2;

    /// Whether the see-online bit is set.
    #[must_use]
    pub const fn can_see_online(self) -> bool {
        self.0 & Self::CAN_SEE_ONLINE != 0
    }

    /// Whether the see-on-map bit is set.
    #[must_use]
    pub const fn can_see_on_map(self) -> bool {
        self.0 & Self::CAN_SEE_ON_MAP != 0
    }

    /// Whether the modify-objects bit is set.
    #[must_use]
    pub const fn can_modify_objects(self) -> bool {
        self.0 & Self::CAN_MODIFY_OBJECTS != 0
    }
}

impl_bitfield_serde!(
    FriendRights,
    i32,
    "CAN_SEE_ONLINE" => FriendRights::CAN_SEE_ONLINE,
    "CAN_SEE_ON_MAP" => FriendRights::CAN_SEE_ON_MAP,
    "CAN_MODIFY_OBJECTS" => FriendRights::CAN_MODIFY_OBJECTS,
);
