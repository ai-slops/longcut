use std::cmp::Ordering;

use crate::{Rounding, TimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rational {
    numerator: i128,
    denominator: i128,
}

impl Rational {
    pub(crate) fn new(numerator: i128, denominator: i128) -> Result<Self, TimeError> {
        if denominator == 0 {
            return Err(TimeError::ZeroDuration);
        }
        if numerator == i128::MIN || denominator == i128::MIN {
            return Err(TimeError::Overflow);
        }
        let (numerator, denominator) = if denominator < 0 {
            (
                numerator.checked_neg().ok_or(TimeError::Overflow)?,
                denominator.checked_neg().ok_or(TimeError::Overflow)?,
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

    pub(crate) fn from_integer(value: i64) -> Self {
        Self {
            numerator: i128::from(value),
            denominator: 1,
        }
    }

    pub(crate) fn checked_add(self, other: Self) -> Result<Self, TimeError> {
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
                    .and_then(|right| left.checked_add(right))
            })
            .ok_or(TimeError::Overflow)?;
        let denominator = self
            .denominator
            .checked_mul(left_scale)
            .ok_or(TimeError::Overflow)?;
        Self::new(numerator, denominator)
    }

    pub(crate) fn checked_sub(self, other: Self) -> Result<Self, TimeError> {
        let negative = Self::new(
            other.numerator.checked_neg().ok_or(TimeError::Overflow)?,
            other.denominator,
        )?;
        self.checked_add(negative)
    }

    pub(crate) fn checked_mul(self, other: Self) -> Result<Self, TimeError> {
        let left_cancel = gcd(self.numerator.abs(), other.denominator);
        let right_cancel = gcd(other.numerator.abs(), self.denominator);
        let numerator = (self.numerator / left_cancel)
            .checked_mul(other.numerator / right_cancel)
            .ok_or(TimeError::Overflow)?;
        let denominator = (self.denominator / right_cancel)
            .checked_mul(other.denominator / left_cancel)
            .ok_or(TimeError::Overflow)?;
        Self::new(numerator, denominator)
    }

    pub(crate) fn checked_div(self, other: Self) -> Result<Self, TimeError> {
        if other.numerator == 0 {
            return Err(TimeError::ZeroDuration);
        }
        self.checked_mul(Self::new(other.denominator, other.numerator)?)
    }

    pub(crate) fn compare(self, other: Self) -> Result<Ordering, TimeError> {
        Ok(self.checked_sub(other)?.numerator.cmp(&0))
    }

    pub(crate) fn round_i64(self, rounding: Rounding) -> Result<i64, TimeError> {
        let quotient = self.numerator / self.denominator;
        let remainder = self.numerator % self.denominator;
        let rounded = match rounding {
            Rounding::Exact if remainder != 0 => return Err(TimeError::Inexact),
            Rounding::Floor if remainder < 0 => quotient.checked_sub(1),
            Rounding::Ceil if remainder > 0 => quotient.checked_add(1),
            Rounding::Nearest if remainder != 0 => {
                let magnitude = remainder.abs();
                if magnitude >= self.denominator - magnitude {
                    quotient.checked_add(remainder.signum())
                } else {
                    Some(quotient)
                }
            }
            Rounding::Exact | Rounding::Floor | Rounding::Ceil | Rounding::Nearest => {
                Some(quotient)
            }
        }
        .ok_or(TimeError::Overflow)?;
        i64::try_from(rounded).map_err(|_| TimeError::Overflow)
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
