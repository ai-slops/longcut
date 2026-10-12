use serde::{Deserialize, Serialize};

use super::MomentVersionRef;

pub const MAX_CHAPTERS: usize = 64;
pub const MAX_TOTAL_BEATS: usize = 512;
pub const MAX_BEATS_PER_CHAPTER: usize = 128;
pub const MAX_REFS_PER_BEAT: usize = 64;
pub const MAX_TOTAL_REFS: usize = 8_192;
pub const MAX_STORY_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TITLE_BYTES: usize = 200;
pub const MAX_PURPOSE_BYTES: usize = 2_000;
pub const MAX_TARGET_FRAMES: u32 = 2_147_483_647;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Story {
    pub version: String,
    pub chapters: Vec<Chapter>,
}

impl Default for Story {
    fn default() -> Self {
        Self {
            version: "0".into(),
            chapters: vec![],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    pub purpose: String,
    pub beats: Vec<Beat>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Beat {
    pub id: String,
    pub title: String,
    pub purpose: String,
    pub target_frames: Option<String>,
    pub moment_refs: Vec<MomentVersionRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoryVersionRecord {
    pub project_id: String,
    pub version: String,
    pub snapshot: Story,
}
