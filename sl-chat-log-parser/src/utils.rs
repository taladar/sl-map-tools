//! Parsing utilities and general parsers

#[cfg(test)]
use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
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
                Ok(time::PrimitiveDateTime::parse(&input, format)
                    .map(time::PrimitiveDateTime::assume_utc)
                    .map_err(|e| Simple::custom(span, format!("{:?}", e)))?)
            },
        )
}

/// a wrapped error in case parsing fails to get proper error output
/// the chumsky errors themselves lack Display and std::error::Error
/// implementations
#[cfg(test)]
#[derive(Debug)]
pub struct ChumskyError {
    /// description of the object we were trying to parse
    pub description: String,
    /// source string for parsing
    pub source: String,
    /// errors encountered during parsing
    pub errors: Vec<chumsky::error::Simple<char>>,
}

#[cfg(test)]
impl std::fmt::Display for ChumskyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.errors {
            let msg = format!(
                "While parsing {}: {}{}, expected {}",
                self.description,
                if e.found().is_some() {
                    "Unexpected token"
                } else {
                    "Unexpected end of input"
                },
                if let Some(label) = e.label() {
                    format!(" while parsing {}", label)
                } else {
                    String::new()
                },
                if e.expected().len() == 0 {
                    "end of input".to_string()
                } else {
                    e.expected()
                        .map(|expected| match expected {
                            Some(expected) => expected.to_string(),
                            None => "end of input".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            );

            let report = Report::build(ReportKind::Error, e.span())
                .with_code(3)
                .with_message(msg)
                .with_label(
                    Label::new(e.span())
                        .with_message(format!(
                            "Unexpected {}",
                            e.found()
                                .map(|c| format!("token {}", c.fg(Color::Red)))
                                .unwrap_or_else(|| "end of input".to_string())
                        ))
                        .with_color(Color::Red),
                );

            let report = match e.reason() {
                chumsky::error::SimpleReason::Unclosed { span, delimiter } => report.with_label(
                    Label::new(span.clone())
                        .with_message(format!(
                            "Unclosed delimiter {}",
                            delimiter.fg(Color::Yellow)
                        ))
                        .with_color(Color::Yellow),
                ),
                chumsky::error::SimpleReason::Unexpected => report,
                chumsky::error::SimpleReason::Custom(msg) => report.with_label(
                    Label::new(e.span())
                        .with_message(format!("{}", msg.fg(Color::Yellow)))
                        .with_color(Color::Yellow),
                ),
            };

            let mut s: Vec<u8> = Vec::new();
            report
                .finish()
                .write(Source::from(&self.source), &mut s)
                .map_err(|_| <std::fmt::Error as std::default::Default>::default())?;
            let Ok(s) = std::str::from_utf8(&s) else {
                tracing::error!("Expected ariadne to produce valid UTF-8");
                return Err(std::fmt::Error);
            };
            write!(f, "{}", s)?;
        }
        Ok(())
    }
}

#[cfg(test)]
impl std::error::Error for ChumskyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
