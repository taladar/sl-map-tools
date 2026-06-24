//! Experience value types: the property bitfield carried in experience info.

/// Experience [`properties`](ExperienceProperties) bit: the experience id is
/// invalid (a placeholder for an `error_ids` entry the grid could not resolve).
pub const PROPERTY_INVALID: i32 = 1 << 0;
/// Experience properties bit: privileged (a Linden-blessed experience).
pub const PROPERTY_PRIVILEGED: i32 = 1 << 3;
/// Experience properties bit: grid-wide scope (vs. land-scoped). Mirrors the
/// viewer's grid-scope notification on a permission request.
pub const PROPERTY_GRID: i32 = 1 << 4;
/// Experience properties bit: the experience is private.
pub const PROPERTY_PRIVATE: i32 = 1 << 5;
/// Experience properties bit: the experience is disabled.
pub const PROPERTY_DISABLED: i32 = 1 << 6;
/// Experience properties bit: the experience is suspended by an admin.
pub const PROPERTY_SUSPENDED: i32 = 1 << 7;

/// The bitfield of experience-info property flags (the `PROPERTY_*` constants).
/// Mirrors the viewer's `LLExperienceCache` property bits, which it notes should
/// track the grid's `experience-api` model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside this module"
)]
pub struct ExperienceProperties(pub i32);

impl ExperienceProperties {
    /// Whether all of the bits in `flag` are set.
    #[must_use]
    pub const fn contains(self, flag: i32) -> bool {
        self.0 & flag == flag
    }

    /// Whether the experience id is invalid ([`PROPERTY_INVALID`]).
    #[must_use]
    pub const fn is_invalid(self) -> bool {
        self.contains(PROPERTY_INVALID)
    }

    /// Whether the experience is privileged ([`PROPERTY_PRIVILEGED`]).
    #[must_use]
    pub const fn is_privileged(self) -> bool {
        self.contains(PROPERTY_PRIVILEGED)
    }

    /// Whether the experience is grid-wide ([`PROPERTY_GRID`]); otherwise it is
    /// land-scoped.
    #[must_use]
    pub const fn is_grid(self) -> bool {
        self.contains(PROPERTY_GRID)
    }

    /// Whether the experience is private ([`PROPERTY_PRIVATE`]).
    #[must_use]
    pub const fn is_private(self) -> bool {
        self.contains(PROPERTY_PRIVATE)
    }

    /// Whether the experience is disabled ([`PROPERTY_DISABLED`]).
    #[must_use]
    pub const fn is_disabled(self) -> bool {
        self.contains(PROPERTY_DISABLED)
    }

    /// Whether the experience is suspended ([`PROPERTY_SUSPENDED`]).
    #[must_use]
    pub const fn is_suspended(self) -> bool {
        self.contains(PROPERTY_SUSPENDED)
    }
}
