use super::*;
use crate::packets::Envelope;
use longcut_session_contract::model::{AssetId, Integer, MediaRef, MediaTime, Range, SessionTime};

fn segment(start: i64, end: i64) -> TimeMapSegment {
    TimeMapSegment {
        media_ref: MediaRef {
            asset_id: AssetId("asset".into()),
            stream_index: 0,
        },
        media_range: Range {
            start: MediaTime {
                ticks: Integer::new(start),
                timebase_num: 1,
                timebase_den: 30,
            },
            end: MediaTime {
                ticks: Integer::new(end),
                timebase_num: 1,
                timebase_den: 30,
            },
        },
        session_range: Range {
            start: SessionTime {
                monotonic_ns: Integer::new(0),
            },
            end: SessionTime {
                monotonic_ns: Integer::new(1_000_000_000),
            },
        },
        confidence: 1.0,
    }
}

fn probe(base: Option<&str>) -> Result<Probe, serde_json::Error> {
    serde_json::from_value(
        serde_json::json!({ "streams": [{ "index": 0, "codec_type": "video", "time_base": base }], "program_version": { "version": "test" } }),
    )
}

#[test]
fn half_open_extent_accepts_equal_endpoints_and_rejects_earlier_start()
-> Result<(), serde_json::Error> {
    let scan = Scan::from([(
        0,
        Envelope {
            start: Some(-1),
            end: Some(30),
            count: 31,
            complete: true,
            ..Envelope::default()
        },
    )]);
    let good = segment(-1, 30);
    let bad = segment(-2, 30);
    let report = compare(&[(0, &good), (1, &bad)], &probe(Some("1/30"))?, &scan);
    assert_eq!(report.segments[0].check, Check::Passed);
    assert_eq!(report.segments[1].check, Check::Failed);
    assert_eq!(report.check, Check::Failed);
    Ok(())
}

#[test]
fn missing_timing_or_clock_cannot_be_inferred_from_partial_observations()
-> Result<(), Box<dyn std::error::Error>> {
    let declared = segment(0, 30);
    let mut scan = Scan::from([(
        0,
        Envelope {
            start: Some(0),
            end: Some(30),
            count: 31,
            complete: false,
            ..Envelope::default()
        },
    )]);
    assert_eq!(
        compare(&[(0, &declared)], &probe(Some("1/30"))?, &scan).check,
        Check::Unverifiable
    );
    scan.get_mut(&0).ok_or("fixture stream")?.complete = true;
    assert_eq!(
        compare(&[(0, &declared)], &probe(None)?, &scan).check,
        Check::Unverifiable
    );
    assert_eq!(
        compare(&[(0, &declared)], &probe(Some("1/30"))?, &Scan::new()).check,
        Check::Unverifiable
    );
    Ok(())
}

#[test]
fn comparison_overflow_is_unverifiable_not_a_pass_or_panic() -> Result<(), serde_json::Error> {
    let mut declared = segment(0, i64::MAX);
    declared.media_range.end.timebase_den = 4_294_967_291;
    let scan = Scan::from([(
        0,
        Envelope {
            start: Some(0),
            end: Some(i64::MAX),
            count: 1,
            complete: true,
            ..Envelope::default()
        },
    )]);
    let report = compare(
        &[(0, &declared)],
        &probe(Some("9223372036854775807/9223372036854775806"))?,
        &scan,
    );
    assert_eq!(report.check, Check::Unverifiable);
    Ok(())
}

#[test]
fn aggregate_never_promotes_missing_or_unverifiable_evidence() {
    assert_eq!(
        aggregate_bounds([Check::Passed, Check::NotPerformed].into_iter()),
        Check::NotPerformed
    );
    assert_eq!(
        aggregate_bounds([Check::Passed, Check::Unverifiable].into_iter()),
        Check::Unverifiable
    );
    assert_eq!(
        aggregate_bounds([Check::Passed, Check::NotDeclared].into_iter()),
        Check::Passed
    );
    assert_eq!(aggregate_bounds(std::iter::empty()), Check::NotDeclared);
}
