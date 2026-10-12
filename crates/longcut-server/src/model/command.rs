use serde::{Deserialize, Serialize};

use super::{
    Camera, MomentEvidenceSelection, MomentNarrativeRole, MomentReviewStatus, MomentVersionRef,
    Operation, Pin, Project, RenderKind, SourceRole,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommandRequest {
    pub command_id: String,
    pub base_revision: String,
    pub command: Command,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    SetBrief {
        brief: String,
    },
    RenameProject {
        title: String,
    },
    SetSourceRole {
        source_id: String,
        role: SourceRole,
    },
    InsertScene {
        source_id: String,
        in_frame: String,
        out_frame: String,
        title: String,
    },
    TrimScene {
        scene_id: String,
        in_frame: String,
        out_frame: String,
    },
    SplitScene {
        scene_id: String,
        offset_frame: String,
    },
    MoveScene {
        scene_id: String,
        before_scene_id: Option<String>,
    },
    DeleteScene {
        scene_id: String,
    },
    UpdateScene {
        scene_id: String,
        title: String,
        purpose: String,
        caption: String,
    },
    SetCamera {
        scene_id: String,
        camera: Option<Camera>,
    },
    LockScene {
        scene_id: String,
        locked: bool,
    },
    CreateMoment {
        source_id: String,
        start_frame: String,
        end_frame: String,
        label: String,
        note: String,
        narrative_role: MomentNarrativeRole,
        evidence: Option<MomentEvidenceSelection>,
    },
    UpdateMoment {
        moment_id: String,
        expected_moment_version: String,
        source_id: String,
        start_frame: String,
        end_frame: String,
        label: String,
        note: String,
        narrative_role: MomentNarrativeRole,
        evidence: Option<MomentEvidenceSelection>,
    },
    ReviewMoment {
        moment_id: String,
        expected_moment_version: String,
        review_status: MomentReviewStatus,
    },
    ArchiveMoment {
        moment_id: String,
        expected_moment_version: String,
    },
    RestoreMoment {
        moment_id: String,
        expected_moment_version: String,
    },
    InsertSceneFromMoment {
        moment_id: String,
        expected_moment_version: String,
        title: String,
    },
    Story {
        action: StoryCommand,
    },
    CreateFeedback {
        scene_id: String,
        start_frame: String,
        end_frame: String,
        body: String,
        pin: Option<Pin>,
        render_id: Option<String>,
    },
    ResolveFeedback {
        feedback_id: String,
        resolved: bool,
    },
    CreateChangeSet {
        title: String,
        intent: String,
        operations: Vec<Operation>,
        evidence: Vec<String>,
    },
    ApplyChangeSet {
        change_set_id: String,
        operation_ids: Vec<String>,
    },
    RejectChangeSet {
        change_set_id: String,
    },
    Undo,
    Redo,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoryCommand {
    CreateChapter {
        expected_story_version: String,
        title: String,
        purpose: String,
        before_chapter_id: Option<String>,
    },
    UpdateChapter {
        expected_story_version: String,
        chapter_id: String,
        title: String,
        purpose: String,
    },
    MoveChapter {
        expected_story_version: String,
        chapter_id: String,
        before_chapter_id: Option<String>,
    },
    DeleteChapter {
        expected_story_version: String,
        chapter_id: String,
    },
    CreateBeat {
        expected_story_version: String,
        chapter_id: String,
        title: String,
        purpose: String,
        target_frames: Option<String>,
        before_beat_id: Option<String>,
    },
    UpdateBeat {
        expected_story_version: String,
        beat_id: String,
        title: String,
        purpose: String,
        target_frames: Option<String>,
    },
    MoveBeat {
        expected_story_version: String,
        beat_id: String,
        chapter_id: String,
        before_beat_id: Option<String>,
    },
    DeleteBeat {
        expected_story_version: String,
        beat_id: String,
    },
    SetBeatMomentRefs {
        expected_story_version: String,
        beat_id: String,
        moment_refs: Vec<MomentVersionRef>,
    },
    SetSceneBeat {
        expected_story_version: String,
        scene_id: String,
        beat_id: Option<String>,
    },
}

impl StoryCommand {
    pub const fn expected_story_version(&self) -> &String {
        match self {
            Self::CreateChapter {
                expected_story_version,
                ..
            }
            | Self::UpdateChapter {
                expected_story_version,
                ..
            }
            | Self::MoveChapter {
                expected_story_version,
                ..
            }
            | Self::DeleteChapter {
                expected_story_version,
                ..
            }
            | Self::CreateBeat {
                expected_story_version,
                ..
            }
            | Self::UpdateBeat {
                expected_story_version,
                ..
            }
            | Self::MoveBeat {
                expected_story_version,
                ..
            }
            | Self::DeleteBeat {
                expected_story_version,
                ..
            }
            | Self::SetBeatMomentRefs {
                expected_story_version,
                ..
            }
            | Self::SetSceneBeat {
                expected_story_version,
                ..
            } => expected_story_version,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateProject {
    pub title: String,
    pub frame_rate: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateRender {
    pub base_revision: String,
    pub kind: RenderKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedProject {
    pub project: Project,
}
