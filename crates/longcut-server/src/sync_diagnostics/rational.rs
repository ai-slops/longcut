use longcut_session_contract::model::MediaTime;

#[derive(Clone, Copy, Debug)]
pub(super) struct Rational {
    numerator: i128,
    denominator: i128,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ArithmeticError {
    Overflow,
    ZeroDuration,
}

impl Rational {
    pub(super) fn new(numerator: i128, denominator: i128) -> Result<Self, ArithmeticError> {
        if denominator == 0 {
            return Err(ArithmeticError::ZeroDuration);
        }
        if numerator == i128::MIN || denominator == i128::MIN {
            return Err(ArithmeticError::Overflow);
        }
        let (numerator, denominator) = if denominator < 0 {
            (
                numerator.checked_neg().ok_or(ArithmeticError::Overflow)?,
                denominator.checked_neg().ok_or(ArithmeticError::Overflow)?,
            )
        } else {
            (numerator, denominator)
        };
        let divisor = gcd(numerator.abs(), denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    pub(super) fn session_ns(value: i64) -> Result<Self, ArithmeticError> {
        Self::new(i128::from(value), 1_000_000_000)
    }

    pub(super) fn media(value: &MediaTime) -> Result<Self, ArithmeticError> {
        let numerator = i128::from(value.ticks.get())
            .checked_mul(i128::from(value.timebase_num))
            .ok_or(ArithmeticError::Overflow)?;
        Self::new(numerator, i128::from(value.timebase_den))
    }

    pub(super) fn checked_sub(self, other: Self) -> Result<Self, ArithmeticError> {
        let shared = gcd(self.denominator, other.denominator);
        let left_scale = other.denominator / shared;
        let right_scale = self.denominator / shared;
        let numerator = self
            .numerator
            .checked_mul(left_scale)
            .and_then(|left| {
                other
                    .numerator
                    .checked_mul(right_scale)
                    .and_then(|right| left.checked_sub(right))
            })
            .ok_or(ArithmeticError::Overflow)?;
        let denominator = self
            .denominator
            .checked_mul(left_scale)
            .ok_or(ArithmeticError::Overflow)?;
        Self::new(numerator, denominator)
    }

    pub(super) fn checked_div(self, other: Self) -> Result<Self, ArithmeticError> {
        if other.numerator == 0 {
            return Err(ArithmeticError::ZeroDuration);
        }
        let left_cancel = gcd(self.numerator.abs(), other.numerator.abs());
        let right_cancel = gcd(self.denominator, other.denominator);
        let numerator = (self.numerator / left_cancel)
            .checked_mul(other.denominator / right_cancel)
            .ok_or(ArithmeticError::Overflow)?;
        let denominator = (self.denominator / right_cancel)
            .checked_mul(other.numerator / left_cancel)
            .ok_or(ArithmeticError::Overflow)?;
        Self::new(numerator, denominator)
    }

    pub(super) fn checked_mul_integer(self, value: i128) -> Result<Self, ArithmeticError> {
        let cancel = gcd(value.abs(), self.denominator);
        let numerator = self
            .numerator
            .checked_mul(value / cancel)
            .ok_or(ArithmeticError::Overflow)?;
        Self::new(numerator, self.denominator / cancel)
    }

    pub(super) fn parts(self) -> (String, String) {
        (self.numerator.to_string(), self.denominator.to_string())
    }
}

const fn gcd(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    if left == 0 { 1 } else { left }
}
