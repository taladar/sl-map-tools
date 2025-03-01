//! Search related types

#[cfg(feature = "chumsky")]
use chumsky::{
    prelude::{just, Simple},
    Parser,
};

/// Search categories
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIs)]
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
pub fn search_category_parser() -> impl Parser<char, SearchCategory, Error = Simple<char>> {
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
            SearchCategory::All => write!(f, "all"),
            SearchCategory::People => write!(f, "people"),
            SearchCategory::Places => write!(f, "places"),
            SearchCategory::Events => write!(f, "events"),
            SearchCategory::Groups => write!(f, "groups"),
            SearchCategory::Wiki => write!(f, "wiki"),
            SearchCategory::Destinations => write!(f, "destinations"),
            SearchCategory::Classifieds => write!(f, "classifieds"),
        }
    }
}

/// Error deserializing SearchCategory from String
#[derive(Debug, Clone)]
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
