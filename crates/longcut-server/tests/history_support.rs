#![allow(dead_code)]

use longcut_server::{
    model::{
        CreateProject, MediaStatus, Moment, MomentEvidence, MomentEvidenceOrigin,
        MomentNarrativeRole, MomentOrigin, MomentProvenance, MomentReviewStatus, MomentTimingBasis,
        Project, Source, SourceRole,
    },
    store::Store,
};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

pub struct Fixture {
    pub _root: tempfile::TempDir,
    pub path: std::path::PathBuf,
    pub store: Store,
    pub project: Project,
}

pub async fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("create temp directory");
    let path = root.path().join("history.sqlite3");
    let store = Store::open(&path).await.expect("open store");
    let project = store
        .create(CreateProject {
            title: "History".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    Fixture {
        _root: root,
        path,
        store,
        project,
    }
}

pub async fn pool(path: &std::path::Path) -> SqlitePool {
    SqlitePool::connect_with(SqliteConnectOptions::new().filename(path))
        .await
        .expect("open test database")
}

pub fn visual_source() -> Source {
    Source {
        id: "source".into(),
        name: "source.mp4".into(),
        role: SourceRole::Program,
        duration_frames: "300".into(),
        width: 1920,
        height: 1080,
        has_audio: true,
        media_url: "/api/v1/media/source.mp4".into(),
        proxy_url: None,
        thumbnail_url: None,
        waveform: vec![],
        status: MediaStatus::Ready,
        error: None,
    }
}

pub fn moment(id: usize, version: &str) -> Moment {
    Moment {
        id: format!("moment-{id:03}"),
        version: version.into(),
        source_id: "source".into(),
        start_frame: "0".into(),
        end_frame: "1".into(),
        label: format!("Moment {id}"),
        note: "n".repeat(4_000),
        narrative_role: MomentNarrativeRole::Insight,
        origin: MomentOrigin::Manual,
        review_status: MomentReviewStatus::Reviewed,
        archived: false,
        evidence: vec![MomentEvidence {
            transcript_id: "transcript".into(),
            cue_ids: vec![format!("cue-{id}")],
            word_ids: vec![],
            quote: "q".repeat(16 * 1024),
            origin: MomentEvidenceOrigin::SubtitleImport,
            start_frame: "0".into(),
            end_frame: "1".into(),
            timing_basis: MomentTimingBasis::Cues,
        }],
        provenance: MomentProvenance {
            created_command_id: "seed".into(),
            updated_command_id: "seed".into(),
            created_revision: version.into(),
            updated_revision: version.into(),
            created_at: "seed".into(),
            updated_at: "seed".into(),
        },
    }
}

pub async fn seed_project(fixture: &mut Fixture, moment_count: usize, revision: &str) {
    fixture.project.revision = revision.into();
    fixture.project.moments = (0..moment_count).map(|id| moment(id, revision)).collect();
    let pool = pool(&fixture.path).await;
    sqlx::query("UPDATE projects SET state=? WHERE id=?")
        .bind(serde_json::to_string(&fixture.project).expect("serialize project"))
        .bind(&fixture.project.id)
        .execute(&pool)
        .await
        .expect("seed project");
    pool.close().await;
}
