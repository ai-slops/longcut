use axum::http::StatusCode;
use longcut_server::model::*;
use longcut_session_contract::model::*;
use serde_json::json;

#[path = "sync_diagnostics_support.rs"]
mod support;
use support::{SegmentSpec, active_ordinary_session, active_session, diagnostics, segment};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn reports_exact_offset_drift_and_rate_for_different_clocks() -> TestResult {
    // Given
    let maps = TimeMaps {
        segments: vec![segment(SegmentSpec {
            asset: "program-file",
            stream: 0,
            media: (30, 269, 1, 30),
            session_ns: (2_000_000_000, 10_000_000_000),
        })],
        extensions: Default::default(),
    };
    let (state, project, session) = active_session(Some(maps)).await?;
    // When
    let (status, body) = diagnostics(&state, &project, &session).await?;
    // Then
    assert_eq!(status, StatusCode::OK);
    let item = &body["sources"][0]["bindings"][0]["segments"][0];
    assert_eq!(
        item["offset_at_start"],
        json!({"status":"exact","numerator":"1","denominator":"1"})
    );
    assert_eq!(
        item["duration_delta"],
        json!({"status":"exact","numerator":"1","denominator":"30"})
    );
    assert_eq!(
        item["relative_rate_ratio"],
        json!({"status":"exact","numerator":"240","denominator":"239"})
    );
    assert_eq!(
        item["relative_rate_ppm"],
        json!({"status":"exact","numerator":"1000000","denominator":"239"})
    );
    assert_eq!(body["relation"], "metadata_relation_unmeasured_alignment");
    assert_eq!(body["adapter_capability"]["measured_av_alignment"], false);
    Ok(())
}

#[tokio::test]
async fn unions_coverage_reports_gaps_and_limits_overlap_to_each_stream() -> TestResult {
    // Given
    let specs = [
        ("program-file", 0, 0, 2),
        ("program-file", 0, 2, 4),
        ("program-file", 0, 3, 5),
        ("program-file", 1, 1, 4),
    ];
    let segments = specs
        .into_iter()
        .map(|(asset, stream, start, end)| {
            segment(SegmentSpec {
                asset,
                stream,
                media: (start * 30, end * 30, 1, 30),
                session_ns: (start * 1_000_000_000, end * 1_000_000_000),
            })
        })
        .collect();
    let (state, project, session) = active_session(Some(TimeMaps {
        segments,
        extensions: Default::default(),
    }))
    .await?;
    // When
    let (_, body) = diagnostics(&state, &project, &session).await?;
    // Then
    assert_eq!(body["coverage"]["ranges"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        body["coverage"]["gaps"],
        json!([{
            "start":{"status":"exact","numerator":"5","denominator":"1"},
            "end":{"status":"exact","numerator":"10","denominator":"1"}
        }])
    );
    assert_eq!(body["overlaps"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        body["health_notes"],
        json!(["observed_metadata_gap_not_media_corruption"])
    );
    Ok(())
}

#[tokio::test]
async fn missing_maps_are_unknown_and_ordinary_probe_is_not_exact() -> TestResult {
    // Given
    let (state, project, session) = active_session(None).await?;
    // When
    let (_, body) = diagnostics(&state, &project, &session).await?;
    // Then
    assert_eq!(body["coverage"]["status"], "unknown");
    assert_eq!(body["coverage"]["reason"], "time_maps_missing");
    assert_eq!(
        body["adapter_capability"]["ordinary_probe"],
        "metadata_only_not_exact"
    );
    assert!(
        body["sources"][0]["editor_binding_available"]
            .as_bool()
            .unwrap_or(false)
    );
    Ok(())
}

#[tokio::test]
async fn ordinary_summary_binding_is_editable_without_exact_time_maps() -> TestResult {
    // Given
    let (state, project, session) = active_ordinary_session().await?;
    // When
    let (_, body) = diagnostics(&state, &project, &session).await?;
    // Then
    let source = &body["sources"][0];
    assert_eq!(source["editor_binding_available"], true);
    assert_eq!(
        source["editor_source_ids"],
        json!(["ordinary-editor-source"])
    );
    assert_eq!(body["coverage"]["status"], "unknown");
    assert_eq!(body["coverage"]["reason"], "time_maps_missing");
    assert_eq!(body["adapter_capability"]["exact_time_maps"], false);
    Ok(())
}

#[tokio::test]
async fn summary_and_exact_mapping_editor_ids_are_deduplicated() -> TestResult {
    // Given
    let (state, project, session) = active_session(None).await?;
    // When
    let (_, body) = diagnostics(&state, &project, &session).await?;
    // Then
    assert_eq!(
        body["sources"][0]["editor_source_ids"],
        json!(["editor-program"])
    );
    Ok(())
}

#[tokio::test]
async fn empty_maps_are_exact_empty_coverage_not_missing_metadata() -> TestResult {
    // Given
    let maps = TimeMaps {
        segments: vec![],
        extensions: Default::default(),
    };
    let (state, project, session) = active_session(Some(maps)).await?;
    // When
    let (_, body) = diagnostics(&state, &project, &session).await?;
    // Then
    assert_eq!(body["coverage"]["status"], "exact");
    assert_eq!(body["coverage"]["ranges"], json!([]));
    assert_eq!(body["coverage"]["gaps"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["adapter_capability"]["exact_time_maps"], true);
    Ok(())
}

#[tokio::test]
async fn excessive_pairwork_is_explicitly_unverifiable() -> TestResult {
    // Given
    let one = SegmentSpec {
        asset: "program-file",
        stream: 0,
        media: (0, 30, 1, 30),
        session_ns: (0, 1_000_000_000),
    };
    let maps = TimeMaps {
        segments: std::iter::repeat_n(segment(one), 1_001).collect(),
        extensions: Default::default(),
    };
    let (state, project, session) = active_session(Some(maps)).await?;
    // When
    let (_, body) = diagnostics(&state, &project, &session).await?;
    // Then
    assert_eq!(body["calculation_status"], "unverifiable");
    assert_eq!(body["limits"]["exceeded"], true);
    assert_eq!(body["coverage"]["reason"], "validation_limit_exceeded");
    assert_eq!(body["sources"], json!([]));
    Ok(())
}

#[tokio::test]
async fn emits_js_unsafe_values_as_reduced_integer_strings() -> TestResult {
    // Given
    let maps = TimeMaps {
        segments: vec![segment(SegmentSpec {
            asset: "program-file",
            stream: 0,
            media: (9_007_199_254_740_993, 9_007_199_254_741_023, 1, 30),
            session_ns: (0, 1_000_000_000),
        })],
        extensions: Default::default(),
    };
    let (state, project, session) = active_session(Some(maps)).await?;
    // When
    let (_, body) = diagnostics(&state, &project, &session).await?;
    // Then
    let offset = &body["sources"][0]["bindings"][0]["segments"][0]["offset_at_start"];
    assert_eq!(offset["numerator"], "-3002399751580331");
    assert_eq!(offset["denominator"], "10");
    assert!(offset["numerator"].is_string() && offset["denominator"].is_string());
    Ok(())
}

#[tokio::test]
async fn endpoint_follows_active_project_membership_after_undo() -> TestResult {
    // Given
    let (state, project, session) = active_session(None).await?;
    state
        .store
        .command(
            &project,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "1".into(),
                command: Command::Undo,
            },
        )
        .await?;
    // When
    let (status, _) = diagnostics(&state, &project, &session).await?;
    // Then
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}
