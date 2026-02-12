//! Money related data types

#[cfg(feature = "chumsky")]
use chumsky::{IterParser as _, Parser, prelude::just, text::digits};

/// represents a L$ amount
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LindenAmount(pub u64);

impl std::fmt::Display for LindenAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(value) = self;
        write!(f, "{value} L$")
    }
}

impl std::ops::Add for LindenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let Self(lhs) = self;
        let Self(rhs) = rhs;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs + rhs)
    }
}

impl std::ops::Sub for LindenAmount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let Self(lhs) = self;
        let Self(rhs) = rhs;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs - rhs)
    }
}

impl std::ops::Mul<u8> for LindenAmount {
    type Output = Self;

    fn mul(self, rhs: u8) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs * u64::from(rhs))
    }
}

impl std::ops::Mul<u16> for LindenAmount {
    type Output = Self;

    fn mul(self, rhs: u16) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs * u64::from(rhs))
    }
}

impl std::ops::Mul<u32> for LindenAmount {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs * u64::from(rhs))
    }
}

impl std::ops::Mul<u64> for LindenAmount {
    type Output = Self;

    fn mul(self, rhs: u64) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs * rhs)
    }
}

impl std::ops::Div<u8> for LindenAmount {
    type Output = Self;

    fn div(self, rhs: u8) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs / u64::from(rhs))
    }
}

impl std::ops::Div<u16> for LindenAmount {
    type Output = Self;

    fn div(self, rhs: u16) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs / u64::from(rhs))
    }
}

impl std::ops::Div<u32> for LindenAmount {
    type Output = Self;

    fn div(self, rhs: u32) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs / u64::from(rhs))
    }
}

impl std::ops::Div<u64> for LindenAmount {
    type Output = Self;

    fn div(self, rhs: u64) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs / rhs)
    }
}

impl std::ops::Rem<u8> for LindenAmount {
    type Output = Self;

    fn rem(self, rhs: u8) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs % u64::from(rhs))
    }
}

impl std::ops::Rem<u16> for LindenAmount {
    type Output = Self;

    fn rem(self, rhs: u16) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs % u64::from(rhs))
    }
}

impl std::ops::Rem<u32> for LindenAmount {
    type Output = Self;

    fn rem(self, rhs: u32) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs % u64::from(rhs))
    }
}

impl std::ops::Rem<u64> for LindenAmount {
    type Output = Self;

    fn rem(self, rhs: u64) -> Self::Output {
        let Self(lhs) = self;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "this results in the exact same arithmetic side-effects as the same operation on integers which is probably the most expected result for the user"
        )]
        Self(lhs % rhs)
    }
}

/// parse a Linden amount
///
/// "L$1234"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn linden_amount_parser<'src>()
-> impl Parser<'src, &'src str, LindenAmount, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("L$")
        .ignore_then(digits(10).collect::<String>())
        .try_map(|x: String, span: chumsky::span::SimpleSpan| {
            Ok(LindenAmount(x.parse().map_err(|e| {
                chumsky::error::Rich::custom(span, format!("{e:?}"))
            })?))
        })
}
