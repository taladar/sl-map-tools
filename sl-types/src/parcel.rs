//! Parcel value types: access-list flags and object-return classifications.

/// The per-entry classification flags on a parcel access (allow) or ban list.
///
/// A bitfield carried by every entry of a parcel access-list reply (alongside
/// the whole-list scope). On Second Life an entry can be flagged as an
/// experience allow/block in addition to the plain access/ban list it belongs
/// to; OpenSim sets the per-entry flags equal to the list's scope. Combine the
/// constants with [`ParcelAccessFlags::union`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct ParcelAccessFlags(pub u32);

impl ParcelAccessFlags {
    /// No flags set.
    pub const NONE: Self = Self(0);
    /// The entry is on the access (allow) list (`AL_ACCESS`, `1 << 0`).
    pub const ACCESS: Self = Self(1 << 0);
    /// The entry is on the ban list (`AL_BAN`, `1 << 1`).
    pub const BAN: Self = Self(1 << 1);
    /// The entry allows an experience (`AL_ALLOW_EXPERIENCE`, `1 << 3`).
    pub const ALLOW_EXPERIENCE: Self = Self(1 << 3);
    /// The entry blocks an experience (`AL_BLOCK_EXPERIENCE`, `1 << 4`).
    pub const BLOCK_EXPERIENCE: Self = Self(1 << 4);

    /// Combines two sets of access flags.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every bit of `other` is set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether no flags are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// The kinds of objects to return or select on a parcel, as the `ReturnType` of
/// a parcel object-return or object-select request. A bitfield: combine the
/// constants with [`ParcelReturnType::union`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct ParcelReturnType(pub u32);

impl ParcelReturnType {
    /// No objects (`RT_NONE`).
    pub const NONE: Self = Self(1 << 0);
    /// Objects owned by the parcel's owner (`RT_OWNER`).
    pub const OWNER: Self = Self(1 << 1);
    /// Objects set to the parcel's group (`RT_GROUP`).
    pub const GROUP: Self = Self(1 << 2);
    /// Objects owned by anyone else (`RT_OTHER`).
    pub const OTHER: Self = Self(1 << 3);
    /// Only the objects in the supplied id list (`RT_LIST`).
    pub const LIST: Self = Self(1 << 4);
    /// Objects that are for sale (`RT_SELL`).
    pub const SELL: Self = Self(1 << 5);

    /// Combines two sets of return-type bits.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
