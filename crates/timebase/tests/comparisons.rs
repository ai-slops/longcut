//! Exact physical-time comparisons without frame or nanosecond rounding.

use longcut_timebase::{SourceTime, TimeBase};
use std::cmp::Ordering;

#[test]
fn compares_equal_physical_times_in_different_clocks() -> Result<(), Box<dyn std::error::Error>> {
    let ntsc = SourceTime::new(60_000, TimeBase::new(1001, 60_000)?);
    let audio = SourceTime::new(48_048_000, TimeBase::new(1, 48_000)?);
    assert_eq!(ntsc.compare(audio)?, Ordering::Equal);
    Ok(())
}

#[test]
fn preserves_subnanosecond_order_including_negative_origins()
-> Result<(), Box<dyn std::error::Error>> {
    let base = TimeBase::new(1, 2_000_000_000)?;
    let half_ns = SourceTime::new(1, base);
    let zero = SourceTime::new(0, TimeBase::new(1, 30)?);
    assert_eq!(half_ns.compare(zero)?, Ordering::Greater);
    assert_eq!(SourceTime::new(-1, base).compare(zero)?, Ordering::Less);
    Ok(())
}

#[test]
fn compares_ticks_beyond_javascript_precision_without_conversion_overflow()
-> Result<(), Box<dyn std::error::Error>> {
    let base = TimeBase::new(1, 1)?;
    let left = SourceTime::new(9_007_199_254_740_992, base);
    let right = SourceTime::new(9_007_199_254_740_993, base);
    assert_eq!(left.compare(right)?, Ordering::Less);
    Ok(())
}
