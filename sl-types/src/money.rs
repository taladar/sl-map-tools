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

/// A signed Second Life L$ value: a balance or a signed transaction amount.
///
/// [`LindenAmount`] models a non-negative quantity (a price, a credit total).
/// Some values are, however, legitimately *signed* — a group's current balance
/// can be negative, a group-accounting transaction is a positive credit or a
/// negative debit, a refund is a delta either way.
///
/// `LindenBalance` is the signed sibling of [`LindenAmount`]: a sign and a
/// non-negative magnitude. The two compose by type — adding a [`LindenAmount`]
/// to a `LindenBalance` is a `LindenBalance`, and a non-negative balance
/// converts back to a [`LindenAmount`] (a negative one is rejected). Zero is
/// always canonically non-negative, so there is no distinct "negative zero"
/// representation and equality matches ordering.
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LindenBalance {
    /// Whether the value is negative. Canonically `false` when the magnitude is
    /// zero, so there is no negative-zero representation.
    negative: bool,
    /// The absolute value, in L$.
    magnitude: LindenAmount,
}

impl LindenBalance {
    /// A zero balance.
    pub const ZERO: Self = Self {
        negative: false,
        magnitude: LindenAmount(0),
    };

    /// Builds a balance from a sign and a magnitude, normalising a zero
    /// magnitude to non-negative so there is no negative-zero representation.
    #[must_use]
    pub const fn new(negative: bool, magnitude: LindenAmount) -> Self {
        let LindenAmount(value) = magnitude;
        Self {
            negative: negative && value != 0,
            magnitude,
        }
    }

    /// Whether this balance is strictly negative.
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        self.negative
    }

    /// Whether this balance is zero.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        let LindenAmount(value) = self.magnitude;
        value == 0
    }

    /// The absolute value of this balance, in L$.
    #[must_use]
    pub fn magnitude(&self) -> LindenAmount {
        self.magnitude.clone()
    }

    /// The value as a 128-bit signed integer — wide enough that the full
    /// `u64` magnitude (positive or negated) never overflows. The basis for
    /// ordering and arithmetic.
    fn signed_i128(&self) -> i128 {
        let LindenAmount(value) = self.magnitude;
        let magnitude = i128::from(value);
        if self.negative {
            magnitude.wrapping_neg()
        } else {
            magnitude
        }
    }

    /// Rebuilds a balance from a 128-bit signed value, saturating the magnitude
    /// at `u64::MAX` (a bound no real L$ value approaches).
    fn from_signed_i128(value: i128) -> Self {
        let magnitude = u64::try_from(value.unsigned_abs()).unwrap_or(u64::MAX);
        Self::new(value.is_negative(), LindenAmount(magnitude))
    }

    /// The value as a 64-bit signed integer, or `None` if the magnitude exceeds
    /// the `i64` range.
    fn signed_i64(&self) -> Option<i64> {
        let LindenAmount(value) = self.magnitude;
        let magnitude = i64::try_from(value).ok()?;
        Some(if self.negative {
            magnitude.wrapping_neg()
        } else {
            magnitude
        })
    }

    /// Decodes a signed 32-bit L$ wire field into a balance. Total: every `i32`
    /// is a valid balance.
    #[must_use]
    pub fn from_i32(value: i32) -> Self {
        Self::new(
            value.is_negative(),
            LindenAmount(u64::from(value.unsigned_abs())),
        )
    }

    /// Encodes a balance into a signed 32-bit L$ wire field, or `None` if the
    /// value is outside the `i32` range a wire field can hold.
    #[must_use]
    pub fn to_i32(&self) -> Option<i32> {
        i32::try_from(self.signed_i64()?).ok()
    }

    /// Decodes a signed 64-bit L$ value into a balance. Total: every `i64` is a
    /// valid balance.
    #[must_use]
    pub const fn from_i64(value: i64) -> Self {
        Self::new(value.is_negative(), LindenAmount(value.unsigned_abs()))
    }

    /// Encodes a balance into a signed 64-bit L$ value, or `None` if the
    /// magnitude exceeds the `i64` range.
    #[must_use]
    pub fn to_i64(&self) -> Option<i64> {
        self.signed_i64()
    }
}

impl std::fmt::Display for LindenBalance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.negative {
            write!(f, "-{}", self.magnitude)
        } else {
            write!(f, "{}", self.magnitude)
        }
    }
}

impl Ord for LindenBalance {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.signed_i128().cmp(&other.signed_i128())
    }
}

impl PartialOrd for LindenBalance {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<LindenAmount> for LindenBalance {
    fn from(amount: LindenAmount) -> Self {
        Self::new(false, amount)
    }
}

/// The error from converting a negative [`LindenBalance`] into a non-negative
/// [`LindenAmount`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("L$ balance {balance} is negative and has no LindenAmount representation")]
#[non_exhaustive]
pub struct NegativeBalanceError {
    /// The negative balance that could not be represented as a [`LindenAmount`].
    pub balance: LindenBalance,
}

impl TryFrom<LindenBalance> for LindenAmount {
    type Error = NegativeBalanceError;

    fn try_from(balance: LindenBalance) -> Result<Self, Self::Error> {
        if balance.negative {
            Err(NegativeBalanceError { balance })
        } else {
            Ok(balance.magnitude)
        }
    }
}

impl std::ops::Add<LindenAmount> for LindenBalance {
    type Output = Self;

    fn add(self, rhs: LindenAmount) -> Self::Output {
        let LindenAmount(value) = rhs;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "operands are bounded by u64::MAX, so the sum fits in i128 without overflow"
        )]
        Self::from_signed_i128(self.signed_i128() + i128::from(value))
    }
}

impl std::ops::Sub<LindenAmount> for LindenBalance {
    type Output = Self;

    fn sub(self, rhs: LindenAmount) -> Self::Output {
        let LindenAmount(value) = rhs;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "operands are bounded by u64::MAX, so the difference fits in i128 without overflow"
        )]
        Self::from_signed_i128(self.signed_i128() - i128::from(value))
    }
}

impl std::ops::Add for LindenBalance {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "operands are bounded by u64::MAX in magnitude, so the sum fits in i128 without overflow"
        )]
        Self::from_signed_i128(self.signed_i128() + rhs.signed_i128())
    }
}

impl std::ops::Sub for LindenBalance {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "operands are bounded by u64::MAX in magnitude, so the difference fits in i128 without overflow"
        )]
        Self::from_signed_i128(self.signed_i128() - rhs.signed_i128())
    }
}

impl std::ops::Neg for LindenBalance {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(!self.negative, self.magnitude)
    }
}

impl std::ops::AddAssign<LindenAmount> for LindenBalance {
    fn add_assign(&mut self, rhs: LindenAmount) {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "delegates to the annotated Add impl, which cannot overflow for u64-bounded operands"
        )]
        {
            *self = self.clone() + rhs;
        }
    }
}

impl std::ops::SubAssign<LindenAmount> for LindenBalance {
    fn sub_assign(&mut self, rhs: LindenAmount) {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "delegates to the annotated Sub impl, which cannot overflow for u64-bounded operands"
        )]
        {
            *self = self.clone() - rhs;
        }
    }
}

impl std::ops::AddAssign for LindenBalance {
    fn add_assign(&mut self, rhs: Self) {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "delegates to the annotated Add impl, which cannot overflow for u64-bounded operands"
        )]
        {
            *self = self.clone() + rhs;
        }
    }
}

impl std::ops::SubAssign for LindenBalance {
    fn sub_assign(&mut self, rhs: Self) {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "delegates to the annotated Sub impl, which cannot overflow for u64-bounded operands"
        )]
        {
            *self = self.clone() - rhs;
        }
    }
}

/// parse a signed Linden balance
///
/// "L$1234" or "-L$1234"
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn linden_balance_parser<'src>()
-> impl Parser<'src, &'src str, LindenBalance, chumsky::extra::Err<chumsky::error::Rich<'src, char>>>
{
    just("-")
        .or_not()
        .then(linden_amount_parser())
        .map(|(sign, magnitude)| LindenBalance::new(sign.is_some(), magnitude))
}

#[cfg(test)]
mod tests {
    use super::{LindenAmount, LindenBalance, NegativeBalanceError};
    use pretty_assertions::assert_eq;

    #[test]
    fn i32_wire_round_trips_bit_identically() {
        for wire in [0_i32, 1, -1, 250, -250, i32::MAX, i32::MIN] {
            let balance = LindenBalance::from_i32(wire);
            assert_eq!(balance.to_i32(), Some(wire));
        }
    }

    #[test]
    fn negative_zero_normalises_to_non_negative() {
        let from_ctor = LindenBalance::new(true, LindenAmount(0));
        assert!(!from_ctor.is_negative());
        assert!(from_ctor.is_zero());
        assert_eq!(from_ctor, LindenBalance::ZERO);
        assert_eq!(from_ctor, LindenBalance::from_i32(0));
    }

    #[test]
    fn ordering_is_by_signed_value() {
        let neg = LindenBalance::from_i32(-100);
        let zero = LindenBalance::ZERO;
        let pos = LindenBalance::from_i32(100);
        assert!(neg < zero);
        assert!(zero < pos);
        assert!(neg < pos);
        // Among negatives, a larger magnitude is the smaller balance.
        assert!(LindenBalance::from_i32(-200) < LindenBalance::from_i32(-100));
    }

    #[test]
    fn amount_and_balance_compose_by_type() {
        let balance = LindenBalance::from_i32(-30);
        assert_eq!(balance + LindenAmount(100), LindenBalance::from_i32(70));
        assert_eq!(
            LindenBalance::from_i32(50) - LindenAmount(80),
            LindenBalance::from_i32(-30)
        );
        assert_eq!(
            LindenBalance::from_i32(10) + LindenBalance::from_i32(-25),
            LindenBalance::from_i32(-15)
        );
        assert_eq!(-LindenBalance::from_i32(42), LindenBalance::from_i32(-42));
    }

    #[test]
    fn linden_amount_interconverts() {
        assert_eq!(
            LindenBalance::from(LindenAmount(500)),
            LindenBalance::from_i32(500)
        );
        assert_eq!(
            LindenAmount::try_from(LindenBalance::from_i32(500)),
            Ok(LindenAmount(500))
        );
        let err = LindenAmount::try_from(LindenBalance::from_i32(-1));
        assert!(matches!(err, Err(NegativeBalanceError { .. })));
    }

    #[test]
    fn out_of_i32_range_does_not_encode() {
        let too_big = LindenBalance::from_i64(i64::from(i32::MAX) + 1);
        assert_eq!(too_big.to_i32(), None);
        assert_eq!(too_big.to_i64(), Some(i64::from(i32::MAX) + 1));
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn balance_parser_round_trips() {
        use chumsky::Parser as _;
        assert_eq!(
            super::linden_balance_parser().parse("L$1234").into_result(),
            Ok(LindenBalance::from_i32(1234))
        );
        assert_eq!(
            super::linden_balance_parser()
                .parse("-L$1234")
                .into_result(),
            Ok(LindenBalance::from_i32(-1234))
        );
    }
}
