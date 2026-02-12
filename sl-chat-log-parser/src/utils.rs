//! Parsing utilities and general parsers

#[cfg(test)]
use ariadne::{Color, Fmt as _, Label, Report, ReportKind, Source};
use chumsky::IterParser as _;
use chumsky::Parser;
use chumsky::prelude::{just, one_of};

/// parse an iso8601 timestamp into a time::OffsetDateTime
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[must_use]
pub fn offset_datetime_parser<'src>() -> impl Parser<
    'src,
    &'src str,
    time::OffsetDateTime,
    chumsky::extra::Err<chumsky::error::Rich<'src, char>>,
> {
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
                let input = format!("{year}-{month}-{day}T{hour}:{minute}:{second}.{microsecond}Z");
                let format = time::macros::format_description!(
                    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
                );
                time::PrimitiveDateTime::parse(&input, format)
                    .map(time::PrimitiveDateTime::assume_utc)
                    .map_err(|e| chumsky::error::Rich::custom(span, format!("{e:?}")))
            },
        )
}

/// a wrapped error in case parsing fails to get proper error output
/// the chumsky errors themselves lack Display and std::error::Error
/// implementations
#[cfg(test)]
#[derive(Debug)]
pub struct ChumskyError<E> {
    /// description of the object we were trying to parse
    pub description: String,
    /// source string for parsing
    pub source: String,
    /// errors encountered during parsing
    pub errors: Vec<E>,
}

#[cfg(test)]
impl std::fmt::Display for ChumskyError<chumsky::error::Rich<'static, char>> {
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
                format_args!(" while parsing {:?}", e.contexts().collect::<Vec<_>>()),
                if e.expected().len() == 0 {
                    "end of input".to_string()
                } else {
                    e.expected()
                        .map(|rich_pattern| rich_pattern.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            );

            let report = Report::build(ReportKind::Error, e.span().start..e.span().end)
                .with_code(3)
                .with_message(msg)
                .with_label(
                    Label::new(e.span().start..e.span().end)
                        .with_message(format!(
                            "Unexpected {}",
                            e.found().map_or_else(
                                || "end of input".to_string(),
                                |c| format!("token {}", c.fg(Color::Red))
                            )
                        ))
                        .with_color(Color::Red),
                );

            let report = match e.reason() {
                chumsky::error::RichReason::ExpectedFound {
                    expected: _,
                    found: _,
                } => report,
                chumsky::error::RichReason::Custom(msg) => report.with_label(
                    Label::new(e.span().start..e.span().end)
                        .with_message(format!("{}", msg.fg(Color::Yellow)))
                        .with_color(Color::Yellow),
                ),
            };

            let mut s: Vec<u8> = Vec::new();
            report
                .finish()
                .write(Source::from(&self.source), &mut s)
                .map_err(|_err| <std::fmt::Error as std::default::Default>::default())?;
            let s = std::str::from_utf8(&s)
                .map_err(|_err| <std::fmt::Error as std::default::Default>::default())?;
            write!(f, "{s}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
impl<E> std::error::Error for ChumskyError<E>
where
    E: std::fmt::Debug,
    Self: std::fmt::Display,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
