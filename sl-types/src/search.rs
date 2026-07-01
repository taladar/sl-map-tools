//! Search related types

#[cfg(feature = "chumsky")]
use chumsky::{Parser, prelude::just};

/// Search categories
#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::EnumIs,
    serde::Serialize,
    serde::Deserialize,
)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is used outside this module"
)]
pub enum SearchCategory {
    /// search in all categories
    All,
    /// search for an avatar
    People,
    /// search for a parcel
    Places,
    /// search for an event
    Events,
    /// search for a group
    Groups,
    /// search the wiki
    Wiki,
    /// search the destination guide
    Destinations,
    /// search the classifieds
    Classifieds,
}

/// parse a search category
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
#[expect(
    clippy::module_name_repetitions,
    reason = "the parser is used outside this module"
)]
pub fn search_category_parser<'src>()
-> impl Parser<'src, &'src str, SearchCategory, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("all")
        .to(SearchCategory::All)
        .or(just("people").to(SearchCategory::People))
        .or(just("places").to(SearchCategory::Places))
        .or(just("events").to(SearchCategory::Events))
        .or(just("groups").to(SearchCategory::Groups))
        .or(just("wiki").to(SearchCategory::Wiki))
        .or(just("destinations").to(SearchCategory::Destinations))
        .or(just("classifieds").to(SearchCategory::Classifieds))
}

impl std::fmt::Display for SearchCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::People => write!(f, "people"),
            Self::Places => write!(f, "places"),
            Self::Events => write!(f, "events"),
            Self::Groups => write!(f, "groups"),
            Self::Wiki => write!(f, "wiki"),
            Self::Destinations => write!(f, "destinations"),
            Self::Classifieds => write!(f, "classifieds"),
        }
    }
}

/// Error deserializing SearchCategory from String
#[derive(Debug, Clone)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is used outside this module"
)]
pub struct SearchCategoryParseError {
    /// the value that could not be parsed
    value: String,
}

impl std::fmt::Display for SearchCategoryParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse as SearchCategory: {}", self.value)
    }
}

impl std::str::FromStr for SearchCategory {
    type Err = SearchCategoryParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "people" => Ok(Self::People),
            "places" => Ok(Self::Places),
            "events" => Ok(Self::Events),
            "groups" => Ok(Self::Groups),
            "wiki" => Ok(Self::Wiki),
            "destinations" => Ok(Self::Destinations),
            "classifieds" => Ok(Self::Classifieds),
            _ => Err(SearchCategoryParseError {
                value: s.to_owned(),
            }),
        }
    }
}

/// The id of an in-world scheduled **event** in the Second Life *events
/// directory* (Search → Events) — the numeric handle an events-directory search
/// result carries and the `secondlife:///app/event/<id>/about` viewer URI
/// references.
///
/// Unlike the UUID-based [keys](crate::key) this is a 32-bit integer (the
/// reference viewer parses the app-URI id with `asInteger()` and the
/// events-directory messages carry it as a `U32`). It is a newtype rather than a
/// bare `u32` so an events-directory id can't be transposed with any other
/// 32-bit field.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct EventId(pub u32);

impl EventId {
    /// Builds an event-directory id from its raw `u32` value.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the raw `u32` value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// parse an event id
///
/// "12345"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn event_id_parser<'src>()
-> impl Parser<'src, &'src str, EventId, chumsky::extra::Err<chumsky::error::Rich<'src, char>>> {
    crate::utils::u32_parser().map(EventId)
}

/// A classified-ad search category — the listing's classified-directory
/// classification (a [`ClassifiedInfo`]/`DirClassifiedQuery` `Category`). The
/// wire value is a `u32` (the viewer's classified category combo; `0` for "any
/// category").
///
/// Not to be confused with a parcel's land classification or with
/// [`SearchCategory`] (the Search-floater tab, a viewer-UI concept).
///
/// [`ClassifiedInfo`]: https://wiki.secondlife.com/wiki/ClassifiedInfoReply
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ClassifiedCategory {
    /// Any category (`0`); the "any" filter in a query, or an unset listing.
    #[default]
    AnyCategory,
    /// Shopping (`1`).
    Shopping,
    /// Land rental (`2`).
    LandRental,
    /// Property rental (`3`).
    PropertyRental,
    /// A special attraction (`4`).
    SpecialAttraction,
    /// New products (`5`).
    NewProducts,
    /// Employment (`6`).
    Employment,
    /// Wanted (`7`).
    Wanted,
    /// A service (`8`).
    Service,
    /// Personal (`9`).
    Personal,
    /// An unrecognised category value, preserved verbatim. Has no named textual
    /// form (its [`Display`](std::fmt::Display) is the raw number).
    Unknown(u32),
}

impl ClassifiedCategory {
    /// Classifies a classified-category wire value.
    #[must_use]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::AnyCategory,
            1 => Self::Shopping,
            2 => Self::LandRental,
            3 => Self::PropertyRental,
            4 => Self::SpecialAttraction,
            5 => Self::NewProducts,
            6 => Self::Employment,
            7 => Self::Wanted,
            8 => Self::Service,
            9 => Self::Personal,
            other => Self::Unknown(other),
        }
    }

    /// The wire value for this category.
    #[must_use]
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::AnyCategory => 0,
            Self::Shopping => 1,
            Self::LandRental => 2,
            Self::PropertyRental => 3,
            Self::SpecialAttraction => 4,
            Self::NewProducts => 5,
            Self::Employment => 6,
            Self::Wanted => 7,
            Self::Service => 8,
            Self::Personal => 9,
            Self::Unknown(value) => value,
        }
    }
}

impl std::fmt::Display for ClassifiedCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AnyCategory => write!(f, "any"),
            Self::Shopping => write!(f, "shopping"),
            Self::LandRental => write!(f, "land rental"),
            Self::PropertyRental => write!(f, "property rental"),
            Self::SpecialAttraction => write!(f, "special attraction"),
            Self::NewProducts => write!(f, "new products"),
            Self::Employment => write!(f, "employment"),
            Self::Wanted => write!(f, "wanted"),
            Self::Service => write!(f, "service"),
            Self::Personal => write!(f, "personal"),
            Self::Unknown(value) => write!(f, "{value}"),
        }
    }
}

impl serde::Serialize for ClassifiedCategory {
    /// Serialized as its raw `u32` wire value, which keeps unrecognised
    /// categories ([`ClassifiedCategory::Unknown`]) lossless across a round
    /// trip.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.to_u32())
    }
}

impl<'de> serde::Deserialize<'de> for ClassifiedCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::from_u32(u32::deserialize(deserializer)?))
    }
}

/// Error deserializing `ClassifiedCategory` from a string
#[derive(Debug, Clone)]
pub struct ClassifiedCategoryParseError {
    /// the value that could not be parsed
    value: String,
}

impl std::fmt::Display for ClassifiedCategoryParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse as ClassifiedCategory: {}", self.value)
    }
}

impl std::str::FromStr for ClassifiedCategory {
    type Err = ClassifiedCategoryParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "any" => Ok(Self::AnyCategory),
            "shopping" => Ok(Self::Shopping),
            "land rental" => Ok(Self::LandRental),
            "property rental" => Ok(Self::PropertyRental),
            "special attraction" => Ok(Self::SpecialAttraction),
            "new products" => Ok(Self::NewProducts),
            "employment" => Ok(Self::Employment),
            "wanted" => Ok(Self::Wanted),
            "service" => Ok(Self::Service),
            "personal" => Ok(Self::Personal),
            _ => Err(ClassifiedCategoryParseError {
                value: s.to_owned(),
            }),
        }
    }
}

/// parse a classified-ad category (the named variants only; the
/// [`Unknown`](ClassifiedCategory::Unknown) catch-all has no textual form)
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn classified_category_parser<'src>() -> impl Parser<
    'src,
    &'src str,
    ClassifiedCategory,
    chumsky::extra::Err<chumsky::error::Rich<'src, char>>,
> {
    just("any")
        .to(ClassifiedCategory::AnyCategory)
        .or(just("shopping").to(ClassifiedCategory::Shopping))
        .or(just("land rental").to(ClassifiedCategory::LandRental))
        .or(just("property rental").to(ClassifiedCategory::PropertyRental))
        .or(just("special attraction").to(ClassifiedCategory::SpecialAttraction))
        .or(just("new products").to(ClassifiedCategory::NewProducts))
        .or(just("employment").to(ClassifiedCategory::Employment))
        .or(just("wanted").to(ClassifiedCategory::Wanted))
        .or(just("service").to(ClassifiedCategory::Service))
        .or(just("personal").to(ClassifiedCategory::Personal))
}
