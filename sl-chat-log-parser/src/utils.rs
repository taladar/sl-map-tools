//! Parsing utilities and general parsers

use chumsky::error::Simple;
use chumsky::prelude::{just, one_of};
use chumsky::Parser;

/// parse an iso8601 timestamp into a time::OffsetDateTime
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn offset_datetime_parser() -> impl Parser<char, time::OffsetDateTime, Error = Simple<char>> {
    one_of("0123456789")
        .repeated()
        .exactly(4)
        .collect::<String>()
        .then_ignore(just('-'))
        .then(
            one_of("0123456789")
                .repeated()
                .exactly(2)
                .collect::<String>(),
        )
        .then_ignore(just('-'))
        .then(
            one_of("0123456789")
                .repeated()
                .exactly(2)
                .collect::<String>(),
        )
        .then_ignore(just('T'))
        .then(
            one_of("0123456789")
                .repeated()
                .exactly(2)
                .collect::<String>(),
        )
        .then_ignore(just(':'))
        .then(
            one_of("0123456789")
                .repeated()
                .exactly(2)
                .collect::<String>(),
        )
        .then_ignore(just(':'))
        .then(
            one_of("0123456789")
                .repeated()
                .exactly(2)
                .collect::<String>(),
        )
        .then_ignore(just('.'))
        .then(
            one_of("0123456789")
                .repeated()
                .exactly(6)
                .collect::<String>(),
        )
        .then_ignore(just('Z'))
        .try_map(
            |((((((year, month), day), hour), minute), second), microsecond), span| {
                let input = format!(
                    "{}-{}-{}T{}:{}:{}.{}Z",
                    year, month, day, hour, minute, second, microsecond
                );
                let format = time::macros::format_description!(
                    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
                );
                Ok(time::OffsetDateTime::parse(&input, format)
                    .map_err(|e| Simple::custom(span, format!("{:?}", e)))?)
            },
        )
}
