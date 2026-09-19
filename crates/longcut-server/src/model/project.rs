use serde::{Deserialize, Serialize};

use super::{ChangeSet, Feedback};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub revision: String,
    pub frame_rate: u32,
    pub updated_at: String,
    pub brief: String,
    pub sources: Vec<Source>,
    pub scenes: Vec<Scene>,
    pub feedback: Vec<Feedback>,
    pub change_sets: Vec<ChangeSet>,
    pub renders: Vec<Render>,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub title: String,
    pub revision: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub role: SourceRole,
    pub duration_frames: String,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    pub media_url: String,
    pub proxy_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub waveform: Vec<f32>,
    pub status: MediaStatus,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceRole {
    Program,
    Primary,
    Camera,
    Audio,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaStatus {
    Processing,
    Ready,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub title: String,
    pub purpose: String,
    pub source_id: String,
    pub in_frame: String,
    pub out_frame: String,
    pub camera: Option<Camera>,
    pub caption: String,
    pub locked: bool,
    pub manual: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Camera {
    pub source_id: String,
    pub in_frame: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub opacity: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Render {
    pub id: String,
    pub revision: String,
    pub kind: RenderKind,
    pub status: RenderStatus,
    pub progress: f32,
    pub media_url: Option<String>,
    pub srt_url: Option<String>,
    pub vtt_url: Option<String>,
    pub lineage_url: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderKind {
    Preview,
    Final,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderStatus {
    Queued,
    Running,
    Ready,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    pub undo: Vec<Project>,
    pub redo: Vec<Project>,
}
