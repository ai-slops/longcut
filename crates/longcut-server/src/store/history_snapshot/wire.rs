use serde::{Deserialize, Serialize};

use crate::model::{
    ChangeSet, Feedback, Moment, MomentVersionRef, Project, Render, Scene, Source,
    SourceSessionSummary,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum SnapshotVersion {
    V1,
}

impl Serialize for SnapshotVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::V1 => serializer.serialize_u8(1),
        }
    }
}

impl<'de> Deserialize<'de> for SnapshotVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            1 => Ok(Self::V1),
            version => Err(serde::de::Error::custom(format!(
                "unsupported project snapshot version {version}"
            ))),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ProjectSnapshot {
    version: SnapshotVersion,
    project_without_moments: ProjectWithoutMoments,
    moment_refs: Vec<MomentVersionRef>,
}

impl ProjectSnapshot {
    pub(super) fn from_project(project: Project) -> Self {
        let moment_refs = project
            .moments
            .iter()
            .map(|moment| MomentVersionRef {
                moment_id: moment.id.clone(),
                version: moment.version.clone(),
            })
            .collect();
        Self {
            version: SnapshotVersion::V1,
            project_without_moments: ProjectWithoutMoments::from_project(project),
            moment_refs,
        }
    }

    pub(super) fn refs(&self) -> &[MomentVersionRef] {
        &self.moment_refs
    }

    pub(super) fn into_project(self, moments: Vec<Moment>) -> Project {
        self.project_without_moments.into_project(moments)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ProjectWithoutMoments {
    id: String,
    title: String,
    revision: String,
    frame_rate: u32,
    updated_at: String,
    brief: String,
    sources: Vec<Source>,
    #[serde(default)]
    source_sessions: Vec<SourceSessionSummary>,
    scenes: Vec<Scene>,
    feedback: Vec<Feedback>,
    change_sets: Vec<ChangeSet>,
    renders: Vec<Render>,
    can_undo: bool,
    can_redo: bool,
}

impl ProjectWithoutMoments {
    fn from_project(project: Project) -> Self {
        Self {
            id: project.id,
            title: project.title,
            revision: project.revision,
            frame_rate: project.frame_rate,
            updated_at: project.updated_at,
            brief: project.brief,
            sources: project.sources,
            source_sessions: project.source_sessions,
            scenes: project.scenes,
            feedback: project.feedback,
            change_sets: project.change_sets,
            renders: project.renders,
            can_undo: project.can_undo,
            can_redo: project.can_redo,
        }
    }

    fn into_project(self, moments: Vec<Moment>) -> Project {
        Project {
            id: self.id,
            title: self.title,
            revision: self.revision,
            frame_rate: self.frame_rate,
            updated_at: self.updated_at,
            brief: self.brief,
            sources: self.sources,
            source_sessions: self.source_sessions,
            moments,
            scenes: self.scenes,
            feedback: self.feedback,
            change_sets: self.change_sets,
            renders: self.renders,
            can_undo: self.can_undo,
            can_redo: self.can_redo,
        }
    }
}
