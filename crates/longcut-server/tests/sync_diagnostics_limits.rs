use longcut_server::{model::SourceSessionRecord, sync_diagnostics::calculate};
use longcut_session_contract::model::{
    Binding, Integer, MediaTime, Source as PackageSource, SourceId, TimeMaps,
};

#[path = "sync_diagnostics_support.rs"]
mod support;
use support::{SegmentSpec, active_ordinary_session, active_session, diagnostics, segment};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

async fn stored_record(time_maps: Option<TimeMaps>) -> TestResult<SourceSessionRecord> {
    let (state, project, session) = active_session(time_maps).await?;
    Ok(state.store.source_session(&project, &session).await?)
}

fn package_sources(template: &PackageSource, shared_stream: bool) -> Vec<PackageSource> {
    (0..142)
        .map(|index| PackageSource {
            source_id: SourceId(format!("source-{index}")),
            display_name: format!("Source {index}"),
            semantic_role: template.semantic_role.clone(),
            kind: template.kind,
            media_bindings: vec![Binding {
                asset_id: template.media_bindings[0].asset_id.clone(),
                stream_index: if shared_stream { 0 } else { index },
                channel_indices: None,
            }],
        })
        .collect()
}

fn segments(shared_stream: bool) -> TimeMaps {
    TimeMaps {
        segments: (0..142)
            .map(|index| {
                segment(SegmentSpec {
                    asset: "program-file",
                    stream: if shared_stream { 0 } else { index },
                    media: (0, 30, 1, 30),
                    session_ns: (0, 1_000_000_000),
                })
            })
            .collect(),
        extensions: Default::default(),
    }
}

#[tokio::test]
async fn projected_shared_stream_rows_are_rejected_before_materialization() -> TestResult {
    // Given
    let (state, project, session) = active_session(Some(segments(true))).await?;
    let (_, baseline) = diagnostics(&state, &project, &session).await?;
    let mut shared = state.store.source_session(&project, &session).await?;
    let template = shared.package.registry.sources[0].clone();
    shared.package.registry.sources = package_sources(&template, true);
    let mut unique = shared.clone();
    unique.package.time_maps = Some(segments(false));
    unique.package.registry.sources = package_sources(&template, false);
    // When
    let shared = serde_json::to_value(calculate(&shared))?;
    let unique = serde_json::to_value(calculate(&unique))?;
    // Then
    assert_eq!(shared["calculation_status"], "unverifiable");
    assert_eq!(shared["limits"]["exceeded"], true);
    assert_eq!(shared["sources"], serde_json::json!([]));
    assert_eq!(baseline["limits"]["exceeded"], false);
    assert_eq!(unique["limits"]["exceeded"], false);
    assert_eq!(unique["sources"].as_array().map(Vec::len), Some(142));
    Ok(())
}

#[tokio::test]
async fn segment_arithmetic_overflow_makes_top_level_unverifiable() -> TestResult {
    // Given
    let mut item = segment(SegmentSpec {
        asset: "program-file",
        stream: 0,
        media: (i64::MIN + 1, i64::MAX, u32::MAX, u32::MAX - 4),
        session_ns: (0, 1_000_000_000),
    });
    item.media_range.end = MediaTime {
        ticks: Integer::new(i64::MAX),
        timebase_num: u32::MAX - 1,
        timebase_den: u32::MAX - 16,
    };
    let record = stored_record(Some(TimeMaps {
        segments: vec![item],
        extensions: Default::default(),
    }))
    .await?;
    // When
    let body = serde_json::to_value(calculate(&record))?;
    // Then
    assert_eq!(body["calculation_status"], "unverifiable");
    assert_eq!(
        body["sources"][0]["bindings"][0]["segments"][0]["duration_delta"]["status"],
        "unverifiable"
    );
    Ok(())
}

#[tokio::test]
async fn unverifiable_declared_duration_makes_top_level_unverifiable() -> TestResult {
    // Given
    let (state, project, session) = active_ordinary_session().await?;
    let mut record = state.store.source_session(&project, &session).await?;
    record.package.time_maps = Some(segments(false));
    record.package.manifest.duration.ticks = Integer::new(1);
    record.package.manifest.duration.timescale = Integer::new(3);
    // When
    let body = serde_json::to_value(calculate(&record))?;
    // Then
    assert_eq!(body["calculation_status"], "unverifiable");
    assert_eq!(body["coverage"]["status"], "unverifiable");
    assert_eq!(body["coverage"]["reason"], "declared_duration_unverifiable");
    Ok(())
}
