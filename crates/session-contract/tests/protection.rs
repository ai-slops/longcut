use longcut_session_contract::{Conformance, model::*, validate};
mod common;
use common::fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cross_product_amplification_is_rejected_before_expansion() -> TestResult {
    // Given
    let mut package = fixture()?;
    let stream = package.registry.assets[1].streams[0].clone();
    for index in 2..2002 {
        let mut stream = stream.clone();
        stream.stream_index = index;
        package.registry.assets[1].streams.push(stream);
    }
    let interval = package
        .media_health
        .as_ref()
        .ok_or("fixture health")?
        .intervals[1]
        .clone();
    package
        .media_health
        .as_mut()
        .ok_or("fixture health")?
        .intervals = vec![interval; 2000];
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code, "VALIDATION_LIMIT_EXCEEDED");
    Ok(())
}

#[test]
fn disjoint_channel_bindings_are_valid() -> TestResult {
    // Given
    let mut package = fixture()?;
    let mut other = package.registry.sources[2].media_bindings[0].clone();
    other.channel_indices = Some(vec![1]);
    package.registry.sources[2].media_bindings.push(other);
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Conformant);
    Ok(())
}

#[test]
fn short_program_reference_is_degraded_even_when_declared_health_matches() -> TestResult {
    // Given
    let mut package = fixture()?;
    let maps = &mut package.time_maps.as_mut().ok_or("fixture maps")?.segments;
    maps[0].session_range.end.monotonic_ns = Integer::new(1_000_000_000);
    maps[0].media_range.end.ticks = Integer::new(30);
    maps[1].session_range.end.monotonic_ns = Integer::new(1_000_000_000);
    maps[1].media_range.end.ticks = Integer::new(48_000);
    package
        .media_health
        .as_mut()
        .ok_or("fixture health")?
        .intervals[0]
        .session_range
        .end
        .monotonic_ns = Integer::new(1_000_000_000);
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Degraded);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "PROGRAM_REFERENCE_COVERAGE_GAP")
    );
    Ok(())
}

#[test]
fn intersecting_channel_bindings_are_rejected() -> TestResult {
    // Given
    let mut package = fixture()?;
    let mut overlapping = package.registry.sources[2].media_bindings[0].clone();
    overlapping.channel_indices = Some(vec![0, 1]);
    package.registry.sources[2].media_bindings.push(overlapping);
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "DUPLICATE_BINDING")
    );
    Ok(())
}

#[test]
fn partially_known_tracks_do_not_claim_full_reconstruction() -> TestResult {
    // Given
    let mut package = fixture()?;
    let visual = &mut package
        .visual_composition
        .as_mut()
        .ok_or("fixture visual")?
        .intervals;
    let mut unknown = visual[0].clone();
    visual[0].session_range.end.monotonic_ns = Integer::new(5_000_000_000);
    unknown.session_range.start.monotonic_ns = Integer::new(5_000_000_000);
    unknown.coverage = Some(Coverage::Partial);
    unknown.contributions.clear();
    visual.push(unknown);
    package.audio_mix.as_mut().ok_or("fixture mix")?.intervals[0].coverage = None;
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Conformant);
    assert!(!report.capabilities.semantic_visual_reconstruction);
    assert!(!report.capabilities.semantic_audio_reconstruction);
    Ok(())
}
