//! Some small helper utilities

#[cfg(feature = "chumsky")]
use chumsky::{
    prelude::{filter, just, one_of, Simple},
    text::digits,
    Parser,
};

/// parse some text in a URL component and URL decode it
///
/// # Errors
///
/// returns and error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn url_text_component_parser() -> impl Parser<char, String, Error = Simple<char>> {
    filter::<char, _, Simple<char>>(|c| {
        c.is_alphabetic() || c.is_numeric() || *c == '%' || *c == '-'
    })
    .repeated()
    .at_least(1)
    .try_map(|s, span| {
        let s = s.into_iter().collect::<String>();
        percent_encoding::percent_decode(s.as_bytes())
            .decode_utf8()
            .map(|s| s.into_owned())
            .map_err(|e| Simple::custom(span, format!("{:?}", e)))
    })
}

/// parse a usize
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn usize_parser() -> impl Parser<char, usize, Error = Simple<char>> {
    digits(10).try_map(|c: String, span| {
        c.parse().map_err(|err| {
            Simple::custom(span, format!("failed to parse {} as usize: {:?}", c, err))
        })
    })
}

/// parse a isize
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn isize_parser() -> impl Parser<char, isize, Error = Simple<char>> {
    one_of("+-")
        .or_not()
        .then(digits(10))
        .try_map(|(sign, c): (Option<char>, String), span| {
            let c = if let Some(sign) = sign {
                format!("{}{}", sign, c)
            } else {
                c
            };
            c.parse().map_err(|err| {
                Simple::custom(span, format!("failed to parse {} as isize: {:?}", c, err))
            })
        })
}

/// parse a u8
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn u8_parser() -> impl Parser<char, u8, Error = Simple<char>> {
    digits(10).try_map(|c: String, span| {
        c.parse()
            .map_err(|err| Simple::custom(span, format!("failed to parse {} as u8: {:?}", c, err)))
    })
}

/// parse a u16
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn u16_parser() -> impl Parser<char, u16, Error = Simple<char>> {
    digits(10).try_map(|c: String, span| {
        c.parse()
            .map_err(|err| Simple::custom(span, format!("failed to parse {} as u16: {:?}", c, err)))
    })
}

/// parse a u32
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn u32_parser() -> impl Parser<char, u32, Error = Simple<char>> {
    digits(10).try_map(|c: String, span| {
        c.parse()
            .map_err(|err| Simple::custom(span, format!("failed to parse {} as u32: {:?}", c, err)))
    })
}

/// parse a u64
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn u64_parser() -> impl Parser<char, u64, Error = Simple<char>> {
    digits(10).try_map(|c: String, span| {
        c.parse()
            .map_err(|err| Simple::custom(span, format!("failed to parse {} as u64: {:?}", c, err)))
    })
}

/// parse a i8
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn i8_parser() -> impl Parser<char, i8, Error = Simple<char>> {
    one_of("+-")
        .or_not()
        .then(digits(10))
        .try_map(|(sign, c): (Option<char>, String), span| {
            let c = if let Some(sign) = sign {
                format!("{}{}", sign, c)
            } else {
                c
            };
            c.parse().map_err(|err| {
                Simple::custom(span, format!("failed to parse {} as i8: {:?}", c, err))
            })
        })
}

/// parse a i16
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn i16_parser() -> impl Parser<char, i16, Error = Simple<char>> {
    one_of("+-")
        .or_not()
        .then(digits(10))
        .try_map(|(sign, c): (Option<char>, String), span| {
            let c = if let Some(sign) = sign {
                format!("{}{}", sign, c)
            } else {
                c
            };
            c.parse().map_err(|err| {
                Simple::custom(span, format!("failed to parse {} as i16: {:?}", c, err))
            })
        })
}

/// parse a i32
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn i32_parser() -> impl Parser<char, i32, Error = Simple<char>> {
    one_of("+-")
        .or_not()
        .then(digits(10))
        .try_map(|(sign, c): (Option<char>, String), span| {
            let c = if let Some(sign) = sign {
                format!("{}{}", sign, c)
            } else {
                c
            };
            c.parse().map_err(|err| {
                Simple::custom(span, format!("failed to parse {} as i32: {:?}", c, err))
            })
        })
}

/// parse a i64
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn i64_parser() -> impl Parser<char, i64, Error = Simple<char>> {
    one_of("+-")
        .or_not()
        .then(digits(10))
        .try_map(|(sign, c): (Option<char>, String), span| {
            let c = if let Some(sign) = sign {
                format!("{}{}", sign, c)
            } else {
                c
            };
            c.parse().map_err(|err| {
                Simple::custom(span, format!("failed to parse {} as i64: {:?}", c, err))
            })
        })
}

/// parse a float without a sign
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn unsigned_f32_parser() -> impl Parser<char, f32, Error = Simple<char>> {
    digits(10).then_ignore(just('.')).then(digits(10)).try_map(
        |(before_point, after_point), span| {
            let raw_float = format!("{}.{}", before_point, after_point);
            raw_float.parse().map_err(|err| {
                Simple::custom(
                    span,
                    format!("Could not parse {} as f32: {:?}", raw_float, err),
                )
            })
        },
    )
}

/// parse a float without a sign
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn unsigned_f64_parser() -> impl Parser<char, f32, Error = Simple<char>> {
    digits(10).then_ignore(just('.')).then(digits(10)).try_map(
        |(before_point, after_point), span| {
            let raw_float = format!("{}.{}", before_point, after_point);
            raw_float.parse().map_err(|err| {
                Simple::custom(
                    span,
                    format!("Could not parse {} as f64: {:?}", raw_float, err),
                )
            })
        },
    )
}

/// parse a float with or without a sign
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn f32_parser() -> impl Parser<char, f32, Error = Simple<char>> {
    one_of("+-")
        .or_not()
        .then(digits(10).then_ignore(just('.')).then(digits(10)))
        .try_map(
            |(sign, (before_point, after_point)): (Option<char>, (String, String)), span| {
                let raw_float = if let Some(sign) = sign {
                    format!("{}{}.{}", sign, before_point, after_point)
                } else {
                    format!("{}.{}", before_point, after_point)
                };
                raw_float.parse().map_err(|err| {
                    Simple::custom(
                        span,
                        format!("Could not parse {} as f32: {:?}", raw_float, err),
                    )
                })
            },
        )
}

/// parse a float with or without a sign
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn f64_parser() -> impl Parser<char, f64, Error = Simple<char>> {
    one_of("+-")
        .or_not()
        .then(digits(10).then_ignore(just('.')).then(digits(10)))
        .try_map(
            |(sign, (before_point, after_point)): (Option<char>, (String, String)), span| {
                let raw_float = if let Some(sign) = sign {
                    format!("{}{}.{}", sign, before_point, after_point)
                } else {
                    format!("{}.{}", before_point, after_point)
                };
                raw_float.parse().map_err(|err| {
                    Simple::custom(
                        span,
                        format!("Could not parse {} as f64: {:?}", raw_float, err),
                    )
                })
            },
        )
}
