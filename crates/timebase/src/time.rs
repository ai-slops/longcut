use serde::{Deserialize, Deserializer, Serialize};

use crate::{TimeError, rational::Rational};

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// Policy used when an exact rational value must become an integer tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rounding {
    /// Reject a fractional result.
    Exact,
    /// Round toward negative infinity.
    Floor,
    /// Round toward positive infinity.
    Ceil,
    /// Round to the nearest integer, with ties away from zero.
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct PositiveRationalWire {
    #[serde(with = "i64_string")]
    numerator: i64,
    #[serde(with = "i64_string")]
    denominator: i64,
}

macro_rules! positive_rational_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        pub struct $name {
            #[serde(with = "i64_string")]
            numerator: i64,
            #[serde(with = "i64_string")]
            denominator: i64,
        }

        impl $name {
            /// Constructs a reduced positive rational.
            pub fn new(numerator: i64, denominator: i64) -> Result<Self, TimeError> {
                if numerator <= 0 || denominator <= 0 {
                    return Err(TimeError::InvalidRate);
                }
                let divisor = gcd_i64(numerator, denominator);
                Ok(Self {
                    numerator: numerator / divisor,
                    denominator: denominator / divisor,
                })
            }

            /// Returns the positive numerator.
            pub const fn numerator(self) -> i64 {
                self.numerator
            }

            /// Returns the positive denominator.
            pub const fn denominator(self) -> i64 {
                self.denominator
            }

            pub(crate) fn ratio(self) -> Result<Rational, TimeError> {
                Rational::new(i128::from(self.numerator), i128::from(self.denominator))
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let wire = PositiveRationalWire::deserialize(deserializer)?;
                Self::new(wire.numerator, wire.denominator).map_err(serde::de::Error::custom)
            }
        }
    };
}

positive_rational_type!(TimeBase, "Seconds represented by one source tick.");
positive_rational_type!(OutputRate, "Output frames represented per second.");

/// Integer nanoseconds in the editing session coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionTime(#[serde(with = "i64_string")] i64);

impl SessionTime {
    /// Constructs a session timestamp from integer nanoseconds.
    pub const fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    /// Returns integer nanoseconds.
    pub const fn ticks(self) -> i64 {
        self.0
    }

    pub(crate) fn ratio(self) -> Rational {
        Rational::from_integer(self.0)
    }
}

/// Integer ticks paired with the source clock that defines their duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceTime {
    #[serde(with = "i64_string")]
    ticks: i64,
    time_base: TimeBase,
}

impl SourceTime {
    /// Constructs a source timestamp.
    pub const fn new(ticks: i64, time_base: TimeBase) -> Self {
        Self { ticks, time_base }
    }

    /// Returns the integer source ticks.
    pub const fn ticks(self) -> i64 {
        self.ticks
    }

    /// Returns the source time base.
    pub const fn time_base(self) -> TimeBase {
        self.time_base
    }

    /// Compares physical time across clocks without rounding to nanoseconds or frames.
    pub fn compare(self, other: Self) -> Result<std::cmp::Ordering, TimeError> {
        self.seconds()?.compare(other.seconds()?)
    }

    /// Converts source ticks to integer session nanoseconds.
    pub fn to_session(self, rounding: Rounding) -> Result<SessionTime, TimeError> {
        let nanos = self
            .seconds()?
            .checked_mul(Rational::new(NANOS_PER_SECOND, 1)?)?;
        Ok(SessionTime::new(nanos.round_i64(rounding)?))
    }

    /// Converts session nanoseconds to ticks in the requested source clock.
    pub fn from_session(
        session: SessionTime,
        time_base: TimeBase,
        rounding: Rounding,
    ) -> Result<Self, TimeError> {
        let seconds = session
            .ratio()
            .checked_div(Rational::new(NANOS_PER_SECOND, 1)?)?;
        let ticks = seconds
            .checked_div(time_base.ratio()?)?
            .round_i64(rounding)?;
        Ok(Self::new(ticks, time_base))
    }

    pub(crate) fn seconds(self) -> Result<Rational, TimeError> {
        Rational::from_integer(self.ticks).checked_mul(self.time_base.ratio()?)
    }
}

/// Integer frame index in an output clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputFrame(#[serde(with = "i64_string")] i64);

impl OutputFrame {
    /// Constructs an output frame index.
    pub const fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    /// Returns the integer output frame index.
    pub const fn ticks(self) -> i64 {
        self.0
    }
}

impl OutputRate {
    /// Converts session nanoseconds to an output frame index.
    pub fn session_to_frame(
        self,
        session: SessionTime,
        rounding: Rounding,
    ) -> Result<OutputFrame, TimeError> {
        let frames = session
            .ratio()
            .checked_mul(self.ratio()?)?
            .checked_div(Rational::new(NANOS_PER_SECOND, 1)?)?;
        Ok(OutputFrame::new(frames.round_i64(rounding)?))
    }

    /// Converts an output frame index to session nanoseconds.
    pub fn frame_to_session(
        self,
        frame: OutputFrame,
        rounding: Rounding,
    ) -> Result<SessionTime, TimeError> {
        let nanos = Rational::from_integer(frame.0)
            .checked_div(self.ratio()?)?
            .checked_mul(Rational::new(NANOS_PER_SECOND, 1)?)?;
        Ok(SessionTime::new(nanos.round_i64(rounding)?))
    }
}

const fn gcd_i64(mut left: i64, mut right: i64) -> i64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

mod i64_string {
    use std::fmt;

    use serde::{Deserializer, Serializer, de::Visitor};

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde with modules require a shared-reference serializer signature"
    )]
    pub(super) fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringIntegerVisitor;

        impl Visitor<'_> for StringIntegerVisitor {
            type Value = i64;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base-10 i64 encoded as a JSON string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(StringIntegerVisitor)
    }
}
