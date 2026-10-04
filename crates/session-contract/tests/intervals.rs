use longcut_session_contract::{Conformance, model::*, validate};
mod common;
use common::fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn range(start: i64, end: i64) -> SessionRange {
    Range {
        start: SessionTime {
            monotonic_ns: Integer::new(start),
        },
        end: SessionTime {
            monotonic_ns: Integer::new(end),
        },
    }
}

#[test]
fn overlapping_time_maps_are_rejected() -> TestResult {
    // Given
    let mut package = fixture()?;
    let maps = &mut package
        .time_maps
        .as_mut()
        .ok_or("fixture time maps")?
        .segments;
    maps.push(maps[0].clone());
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|entry| entry.code == "INVALID_TIME_MAP")
    );
    Ok(())
}

#[test]
fn asset_and_stream_health_overlap_is_rejected() -> TestResult {
    // Given
    let mut package = fixture()?;
    let health = &mut package
        .media_health
        .as_mut()
        .ok_or("fixture health")?
        .intervals;
    let mut duplicate = health[0].clone();
    duplicate.media_ref.stream_index = Some(0);
    health.push(duplicate);
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|entry| entry.code == "HEALTH_OVERLAP")
    );
    Ok(())
}

#[test]
fn explicit_unknown_visual_interval_is_not_a_coverage_gap() -> TestResult {
    // Given
    let mut package = fixture()?;
    let visual = &mut package
        .visual_composition
        .as_mut()
        .ok_or("fixture visuals")?
        .intervals;
    visual[0].contributions.clear();
    visual[0].coverage = Some(Coverage::Partial);
    visual[0].confidence = 0.0;
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Conformant);
    assert!(!report.capabilities.semantic_visual_reconstruction);
    Ok(())
}

#[test]
fn reference_video_with_independent_audio_is_not_independent_video() -> TestResult {
    // Given
    let mut package = fixture()?;
    package.registry.sources[1].kind = SourceKind::Av;
    package.registry.sources[1].media_bindings = vec![
        package.registry.sources[0].media_bindings[0].clone(),
        package.registry.sources[2].media_bindings[0].clone(),
    ];
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|entry| entry.code == "INVALID_VISUAL_SOURCE")
    );
    Ok(())
}

#[test]
fn discontinuity_with_explicit_health_remains_usable_degraded_metadata() -> TestResult {
    // Given
    let mut package = fixture()?;
    let maps = &mut package
        .time_maps
        .as_mut()
        .ok_or("fixture time maps")?
        .segments;
    let mut next = maps[2].clone();
    maps[2].session_range = range(0, 5_000_000_000);
    maps[2].media_range.end.ticks = Integer::new(150);
    next.session_range = range(6_000_000_000, 10_000_000_000);
    next.media_range.start.ticks = Integer::new(180);
    maps.push(next);
    let health = &mut package
        .media_health
        .as_mut()
        .ok_or("fixture health")?
        .intervals;
    let mut audio = health[1].clone();
    audio.media_ref.stream_index = Some(1);
    health[1].media_ref.stream_index = Some(0);
    health[1].session_range = range(0, 5_000_000_000);
    let mut gap = health[1].clone();
    gap.session_range = range(5_000_000_000, 6_000_000_000);
    gap.status = HealthStatus::TimestampGap;
    let mut end = health[1].clone();
    end.session_range = range(6_000_000_000, 10_000_000_000);
    health.extend([audio, gap, end]);
    // When
    let report = validate(&package);
    // Then
    assert_eq!(
        report.metadata_conformance,
        Conformance::Degraded,
        "{:?}",
        report.diagnostics
    );
    assert!(report.capabilities.session_time_map);
    assert!(report.capabilities.media_health_ranges);
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|entry| entry.code == "HEALTH_GAP_UNDECLARED")
    );
    Ok(())
}

#[test]
fn composition_overlap_is_rejected_and_adjacent_intervals_are_valid() -> TestResult {
    // Given
    let mut package = fixture()?;
    let visuals = &mut package
        .visual_composition
        .as_mut()
        .ok_or("fixture visuals")?
        .intervals;
    let mut second = visuals[0].clone();
    visuals[0].session_range = range(0, 5_000_000_000);
    second.session_range = range(5_000_000_000, 10_000_000_000);
    visuals.push(second);
    // When / Then
    assert_eq!(
        validate(&package).metadata_conformance,
        Conformance::Conformant
    );
    package
        .visual_composition
        .as_mut()
        .ok_or("fixture visuals")?
        .intervals[1]
        .session_range = range(4_000_000_000, 10_000_000_000);
    let report = validate(&package);
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|entry| entry.code == "TRACK_OVERLAP")
    );
    Ok(())
}
