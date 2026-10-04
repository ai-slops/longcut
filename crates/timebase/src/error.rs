//! Typed failures from time conversion and piecewise map construction.

use thiserror::Error;

/// Failure from constructing or converting an exact time value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TimeError {
    /// A rational component was zero or negative.
    #[error("rational numerator and denominator must both be positive")]
    InvalidRate,
    /// Checked integer arithmetic exceeded the supported range.
    #[error("time arithmetic overflowed")]
    Overflow,
    /// Exact rounding was requested for a fractional result.
    #[error("time conversion is not exact")]
    Inexact,
    /// A division would have used a zero divisor.
    #[error("time ratio has zero duration")]
    ZeroDuration,
}

/// Failure from constructing a stream key, segment, or time map.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MapError {
    /// An asset or stream identifier was empty.
    #[error("asset and stream identifiers must not be empty")]
    EmptyStreamKey,
    /// A media range was empty or decreasing.
    #[error("media range must increase")]
    InvalidMediaRange,
    /// A session range was empty or decreasing.
    #[error("session range must increase")]
    InvalidSessionRange,
    /// Media ranges overlap for one asset stream.
    #[error("media ranges overlap for {asset}/{stream}")]
    MediaOverlap {
        /// Asset identifier involved in the conflict.
        asset: String,
        /// Stream identifier involved in the conflict.
        stream: String,
    },
    /// Session ranges overlap for one asset stream.
    #[error("session ranges overlap for {asset}/{stream}")]
    SessionOverlap {
        /// Asset identifier involved in the conflict.
        asset: String,
        /// Stream identifier involved in the conflict.
        stream: String,
    },
    /// Media and session ranges progress in opposite orders.
    #[error("media and session ranges reverse order for {asset}/{stream}")]
    ReversedOrder {
        /// Asset identifier involved in the conflict.
        asset: String,
        /// Stream identifier involved in the conflict.
        stream: String,
    },
    /// Exact rational arithmetic failed.
    #[error(transparent)]
    Time(#[from] TimeError),
}
