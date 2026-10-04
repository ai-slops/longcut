use longcut_timebase::{Rounding, SourceTime, TimeBase, TimeError};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
#[schemars(transparent)]
pub struct Integer(#[schemars(with = "String", regex(pattern = r"^-?(0|[1-9][0-9]*)$"))] i64);

impl Integer {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl Serialize for Integer {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Integer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let number: i64 = text.parse().map_err(serde::de::Error::custom)?;
        if number.to_string() != text {
            return Err(serde::de::Error::custom(
                "expected canonical decimal i64 string",
            ));
        }
        Ok(Self(number))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Duration {
    pub ticks: Integer,
    pub timescale: Integer,
}

impl Duration {
    pub fn nanoseconds(&self) -> Result<i64, TimeError> {
        if self.ticks.get() <= 0 {
            return Err(TimeError::InvalidRate);
        }
        SourceTime::new(self.ticks.get(), TimeBase::new(1, self.timescale.get())?)
            .to_session(Rounding::Exact)
            .map(|time| time.ticks())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rational {
    pub numerator: Integer,
    pub denominator: Integer,
}

impl Rational {
    pub fn time_base(&self) -> Result<TimeBase, TimeError> {
        TimeBase::new(self.numerator.get(), self.denominator.get())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MediaTime {
    pub ticks: Integer,
    pub timebase_num: u32,
    pub timebase_den: u32,
}

impl MediaTime {
    pub fn source_time(&self) -> Result<SourceTime, TimeError> {
        Ok(SourceTime::new(
            self.ticks.get(),
            TimeBase::new(i64::from(self.timebase_num), i64::from(self.timebase_den))?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionTime {
    pub monotonic_ns: Integer,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Range<T> {
    pub start: T,
    pub end: T,
}

pub type SessionRange = Range<SessionTime>;

impl SessionRange {
    pub const fn bounds(&self) -> (i64, i64) {
        (self.start.monotonic_ns.get(), self.end.monotonic_ns.get())
    }
}
