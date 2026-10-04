//! Regression coverage for strict JSON time boundaries.

#![allow(
    clippy::expect_used,
    reason = "test fixtures fail most clearly at the construction site"
)]

use longcut_timebase::{OutputRate, SessionTime, SourceTime, TimeBase};

#[test]
fn json_roundtrip_keeps_i64_ticks_as_strings() {
    // Given
    let source = SourceTime::new(
        9_007_199_254_740_993,
        TimeBase::new(1, 1_000_000_000).expect("valid time base"),
    );

    // When
    let json = serde_json::to_string(&source).expect("serialize source time");
    let restored: SourceTime = serde_json::from_str(&json).expect("deserialize source time");
    let session_json =
        serde_json::to_string(&SessionTime::new(i64::MIN)).expect("serialize session time");

    // Then
    assert_eq!(restored, source);
    assert!(json.contains("\"ticks\":\"9007199254740993\""));
    assert_eq!(session_json, "\"-9223372036854775808\"");
}

#[test]
fn json_rejects_numeric_or_malformed_ticks() {
    // Given / When
    let numeric = serde_json::from_str::<SessionTime>("12");
    let malformed = serde_json::from_str::<SessionTime>("\"12.5\"");

    // Then
    assert!(numeric.is_err());
    assert!(malformed.is_err());
}

#[test]
fn json_rejects_zero_negative_and_malformed_rates() {
    // Given / When
    let zero = serde_json::from_str::<TimeBase>(r#"{"numerator":"0","denominator":"1"}"#);
    let negative = serde_json::from_str::<OutputRate>(r#"{"numerator":"-1","denominator":"1"}"#);
    let malformed =
        serde_json::from_str::<OutputRate>(r#"{"numerator":"60000","denominator":"invalid"}"#);
    let numeric = serde_json::from_str::<OutputRate>(r#"{"numerator":60000,"denominator":1001}"#);

    // Then
    assert!(zero.is_err());
    assert!(negative.is_err());
    assert!(malformed.is_err());
    assert!(numeric.is_err());
}
