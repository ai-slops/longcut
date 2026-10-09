use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use longcut_server::{AppState, app, model::*};
use longcut_session_contract::{Conformance, model::*};
use serde_json::Value;
use tower::ServiceExt;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
pub struct SegmentSpec {
    pub asset: &'static str,
    pub stream: u32,
    pub media: (i64, i64, u32, u32),
    pub session_ns: (i64, i64),
}

pub fn segment(spec: SegmentSpec) -> TimeMapSegment {
    let media_time = |ticks| MediaTime {
        ticks: Integer::new(ticks),
        timebase_num: spec.media.2,
        timebase_den: spec.media.3,
    };
    TimeMapSegment {
        media_ref: MediaRef {
            asset_id: AssetId(spec.asset.into()),
            stream_index: spec.stream,
        },
        media_range: Range {
            start: media_time(spec.media.0),
            end: media_time(spec.media.1),
        },
        session_range: Range {
            start: SessionTime {
                monotonic_ns: Integer::new(spec.session_ns.0),
            },
            end: SessionTime {
                monotonic_ns: Integer::new(spec.session_ns.1),
            },
        },
        confidence: 1.0,
    }
}

pub async fn active_session(time_maps: Option<TimeMaps>) -> TestResult<(AppState, String, String)> {
    let source = SessionSource {
        source_id: "program".into(),
        display_name: "Program reference".into(),
        semantic_role: "video.program_reference".into(),
        kind: SourceKind::Av,
        editor_source_id: Some("editor-program".into()),
        reason: None,
    };
    let mapping = EditorSourceMapping {
        editor_source_id: "editor-program".into(),
        package_source_id: "program".into(),
        bindings: vec![],
        session_range: Range {
            start: SessionTime {
                monotonic_ns: Integer::new(0),
            },
            end: SessionTime {
                monotonic_ns: Integer::new(10_000_000_000),
            },
        },
    };
    active_record(time_maps, vec![source], vec![mapping]).await
}

pub async fn active_ordinary_session() -> TestResult<(AppState, String, String)> {
    active_record(
        None,
        vec![SessionSource {
            source_id: "program".into(),
            display_name: "Program reference".into(),
            semantic_role: "video.program_reference".into(),
            kind: SourceKind::Av,
            editor_source_id: Some("ordinary-editor-source".into()),
            reason: None,
        }],
        vec![],
    )
    .await
}

async fn active_record(
    time_maps: Option<TimeMaps>,
    summary_sources: Vec<SessionSource>,
    mappings: Vec<EditorSourceMapping>,
) -> TestResult<(AppState, String, String)> {
    let root = tempfile::tempdir()?.keep();
    let state = AppState::initialize(&root).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "sync".into(),
            frame_rate: 30,
        })
        .await?;
    let job = state.store.create_import(&project.id).await?;
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/edit-session-packages/complete");
    let (mut package, _) = longcut_session_contract::load(&fixture)
        .map_err(|_| "fixture metadata")?
        .into_parts();
    package.time_maps = time_maps;
    let editor_sources = summary_sources
        .iter()
        .filter_map(|source| source.editor_source_id.as_ref())
        .map(|id| longcut_server::model::Source {
            id: id.clone(),
            name: "original".into(),
            role: SourceRole::Program,
            duration_frames: "300".into(),
            width: 160,
            height: 90,
            has_audio: true,
            media_url: "/api/v1/media/example.mp4".into(),
            proxy_url: None,
            thumbnail_url: None,
            waveform: vec![],
            status: MediaStatus::Ready,
            error: None,
        })
        .collect();
    let prepared = PreparedImport {
        record: SourceSessionRecord {
            summary: SourceSessionSummary {
                id: job.id.clone(),
                session_id: package.manifest.session_id.clone(),
                title: "Imported".into(),
                duration_ns: "10000000000".into(),
                conformance: Conformance::Conformant,
                imported_at: job.created_at.clone(),
                assets: vec![],
                sources: summary_sources,
                warnings: vec![],
                verification_url: None,
            },
            package,
            mappings,
        },
        sources: editor_sources,
        artifacts: vec![],
    };
    state.store.prepare_import(&job, &prepared).await?;
    let job = state
        .store
        .transition_import(&project.id, &job.id, ImportStatus::Publishing, None)
        .await?;
    state.store.commit_import(&job, &prepared, "0").await?;
    Ok((state, project.id, job.id))
}

pub async fn diagnostics(
    state: &AppState,
    project: &str,
    session: &str,
) -> TestResult<(StatusCode, Value)> {
    let response = app(state.clone())
        .oneshot(
            Request::get(format!(
                "/api/v1/projects/{project}/source-sessions/{session}/sync-diagnostics"
            ))
            .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let body = serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
    Ok((status, body))
}
