//! Money related data types

/// represents a L$ amount
#[derive(Debug, Clone, Hash)]
pub struct LindenAmount(pub u64);

impl std::fmt::Display for LindenAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let LindenAmount(value) = self;
        write!(f, "{} L$", value)
    }
}

impl std::ops::Add for LindenAmount {
    type Output = LindenAmount;

    fn add(self, rhs: Self) -> Self::Output {
        let LindenAmount(lhs) = self;
        let LindenAmount(rhs) = rhs;
        LindenAmount(lhs + rhs)
    }
}

impl std::ops::Sub for LindenAmount {
    type Output = LindenAmount;

    fn sub(self, rhs: Self) -> Self::Output {
        let LindenAmount(lhs) = self;
        let LindenAmount(rhs) = rhs;
        LindenAmount(lhs - rhs)
    }
}

impl std::ops::Mul<u8> for LindenAmount {
    type Output = LindenAmount;

    fn mul(self, rhs: u8) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs * (rhs as u64))
    }
}

impl std::ops::Mul<u16> for LindenAmount {
    type Output = LindenAmount;

    fn mul(self, rhs: u16) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs * (rhs as u64))
    }
}

impl std::ops::Mul<u32> for LindenAmount {
    type Output = LindenAmount;

    fn mul(self, rhs: u32) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs * (rhs as u64))
    }
}

impl std::ops::Mul<u64> for LindenAmount {
    type Output = LindenAmount;

    fn mul(self, rhs: u64) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs * rhs)
    }
}

impl std::ops::Div<u8> for LindenAmount {
    type Output = LindenAmount;

    fn div(self, rhs: u8) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs / (rhs as u64))
    }
}

impl std::ops::Div<u16> for LindenAmount {
    type Output = LindenAmount;

    fn div(self, rhs: u16) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs / (rhs as u64))
    }
}

impl std::ops::Div<u32> for LindenAmount {
    type Output = LindenAmount;

    fn div(self, rhs: u32) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs / (rhs as u64))
    }
}

impl std::ops::Div<u64> for LindenAmount {
    type Output = LindenAmount;

    fn div(self, rhs: u64) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs / rhs)
    }
}

impl std::ops::Rem<u8> for LindenAmount {
    type Output = LindenAmount;

    fn rem(self, rhs: u8) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs % (rhs as u64))
    }
}

impl std::ops::Rem<u16> for LindenAmount {
    type Output = LindenAmount;

    fn rem(self, rhs: u16) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs % (rhs as u64))
    }
}

impl std::ops::Rem<u32> for LindenAmount {
    type Output = LindenAmount;

    fn rem(self, rhs: u32) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs % (rhs as u64))
    }
}

impl std::ops::Rem<u64> for LindenAmount {
    type Output = LindenAmount;

    fn rem(self, rhs: u64) -> Self::Output {
        let LindenAmount(lhs) = self;
        LindenAmount(lhs % rhs)
    }
}
