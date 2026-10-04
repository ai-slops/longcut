//! Exact media, session, and output time conversion primitives.

mod error;
mod map;
mod rational;
mod time;

pub use error::{MapError, TimeError};
pub use map::{MediaPoint, Segment, SegmentSpec, StreamKey, TimeMap};
pub use time::{OutputFrame, OutputRate, Rounding, SessionTime, SourceTime, TimeBase};
