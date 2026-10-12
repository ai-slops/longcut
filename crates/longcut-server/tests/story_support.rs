#![allow(dead_code)]

use longcut_server::{
    model::{
        Command, CommandRequest, CreateProject, MediaStatus, Project, Source, SourceRole,
        StoryCommand,
    },
    store::Store,
};

pub struct Fixture {
    pub root: tempfile::TempDir,
    pub store: Store,
    pub project: Project,
}

pub async fn fixture() -> Fixture {
    // Given
    let root = tempfile::tempdir().expect("create temp directory");
    let store = Store::open(&root.path().join("longcut.sqlite3"))
        .await
        .expect("open store");
    let project = store
        .create(CreateProject {
            title: "Story".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    Fixture {
        root,
        store,
        project,
    }
}

pub async fn command(
    fixture: &Fixture,
    project: &Project,
    command: Command,
) -> Result<Project, longcut_server::error::AppError> {
    fixture
        .store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: project.revision.clone(),
                command,
            },
        )
        .await
}

pub fn source() -> Source {
    Source {
        id: "source".into(),
        name: "source.mp4".into(),
        role: SourceRole::Program,
        duration_frames: "300".into(),
        width: 1920,
        height: 1080,
        has_audio: true,
        media_url: String::new(),
        proxy_url: None,
        thumbnail_url: None,
        waveform: vec![],
        status: MediaStatus::Ready,
        error: None,
    }
}

pub fn story(action: StoryCommand) -> Command {
    Command::Story { action }
}

pub async fn chapter(fixture: &Fixture, project: &Project, title: &str) -> Project {
    command(
        fixture,
        project,
        story(StoryCommand::CreateChapter {
            expected_story_version: project.story.version.clone(),
            title: title.into(),
            purpose: String::new(),
            before_chapter_id: None,
        }),
    )
    .await
    .expect("create chapter")
}

pub async fn beat(fixture: &Fixture, project: &Project, chapter_id: &str) -> Project {
    command(
        fixture,
        project,
        story(StoryCommand::CreateBeat {
            expected_story_version: project.story.version.clone(),
            chapter_id: chapter_id.into(),
            title: "Beat".into(),
            purpose: String::new(),
            target_frames: None,
            before_beat_id: None,
        }),
    )
    .await
    .expect("create beat")
}
