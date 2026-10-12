#![cfg(test)]

use crate::model::{
    Beat, Chapter, MediaStatus, MomentVersionRef, Project, Scene, Source, SourceRole, Story,
};

#[test]
fn lineage_keeps_frozen_moment_version_reference() {
    // Given
    let project = Project {
        id: "project".into(),
        title: "Moment lineage".into(),
        revision: "9".into(),
        frame_rate: 30,
        updated_at: String::new(),
        brief: String::new(),
        sources: vec![Source {
            id: "source".into(),
            name: "source.mp4".into(),
            role: SourceRole::Program,
            duration_frames: "100".into(),
            width: 1920,
            height: 1080,
            has_audio: true,
            media_url: String::new(),
            proxy_url: None,
            thumbnail_url: None,
            waveform: vec![],
            status: MediaStatus::Ready,
            error: None,
        }],
        source_sessions: vec![],
        moments: vec![],
        story: Story::default(),
        scenes: vec![Scene {
            id: "scene".into(),
            title: "Scene".into(),
            purpose: String::new(),
            source_id: "source".into(),
            in_frame: "10".into(),
            out_frame: "20".into(),
            camera: None,
            caption: String::new(),
            locked: false,
            manual: true,
            moment: Some(MomentVersionRef {
                moment_id: "moment".into(),
                version: "7".into(),
            }),
            beat_id: None,
        }],
        feedback: vec![],
        change_sets: vec![],
        renders: vec![],
        can_undo: false,
        can_redo: false,
    };
    // When
    // SAFE-EXPECT: the test fixture contains a valid bounded scene range.
    let (_, _, lineage) = super::sidecar::build(&project, "render").expect("build lineage");
    // SAFE-EXPECT: the lineage DTO derives Serialize.
    let value = serde_json::to_value(lineage).expect("serialize lineage");
    // Then
    assert_eq!(value["scenes"][0]["moment_id"], "moment");
    assert_eq!(value["scenes"][0]["moment_version"], "7");
    assert_eq!(
        value["scenes"][0]["moment_version_url"],
        "/api/v1/projects/project/moments/moment/versions/7"
    );
}

#[test]
fn lineage_uses_frozen_story_names_and_version() {
    // Given
    let mut project = project_with_scene();
    project.story = Story {
        version: "7".into(),
        chapters: vec![Chapter {
            id: "chapter".into(),
            title: "Frozen chapter".into(),
            purpose: String::new(),
            beats: vec![Beat {
                id: "beat".into(),
                title: "Frozen beat".into(),
                purpose: String::new(),
                target_frames: None,
                moment_refs: vec![],
            }],
        }],
    };
    project.scenes[0].beat_id = Some("beat".into());

    // When
    // SAFE-EXPECT: the fixture contains a valid bounded scene range.
    let (_, _, lineage) = super::sidecar::build(&project, "render").expect("build lineage");
    // SAFE-EXPECT: the lineage DTO derives Serialize.
    let value = serde_json::to_value(lineage).expect("serialize lineage");

    // Then
    assert_eq!(value["story_version"], "7");
    assert_eq!(
        value["story_version_url"],
        "/api/v1/projects/project/story/versions/7"
    );
    assert_eq!(value["scenes"][0]["chapter_title"], "Frozen chapter");
    assert_eq!(value["scenes"][0]["beat_title"], "Frozen beat");
}

#[test]
fn legacy_lineage_serializes_story_fields_as_null() {
    // Given
    let project = project_with_scene();

    // When
    // SAFE-EXPECT: the fixture contains a valid bounded scene range.
    let (_, _, lineage) = super::sidecar::build(&project, "render").expect("build lineage");
    // SAFE-EXPECT: the lineage DTO derives Serialize.
    let value = serde_json::to_value(lineage).expect("serialize lineage");

    // Then
    assert!(value["story_version"].is_null());
    assert!(value["scenes"][0]["beat_id"].is_null());
    assert!(value["scenes"][0]["chapter_id"].is_null());
}

fn project_with_scene() -> Project {
    Project {
        id: "project".into(),
        title: "Story lineage".into(),
        revision: "9".into(),
        frame_rate: 30,
        updated_at: String::new(),
        brief: String::new(),
        sources: vec![Source {
            id: "source".into(),
            name: "source.mp4".into(),
            role: SourceRole::Program,
            duration_frames: "100".into(),
            width: 1920,
            height: 1080,
            has_audio: true,
            media_url: String::new(),
            proxy_url: None,
            thumbnail_url: None,
            waveform: vec![],
            status: MediaStatus::Ready,
            error: None,
        }],
        source_sessions: vec![],
        moments: vec![],
        story: Story::default(),
        scenes: vec![Scene {
            id: "scene".into(),
            title: "Scene".into(),
            purpose: String::new(),
            source_id: "source".into(),
            in_frame: "10".into(),
            out_frame: "20".into(),
            camera: None,
            caption: String::new(),
            locked: false,
            manual: true,
            moment: None,
            beat_id: None,
        }],
        feedback: vec![],
        change_sets: vec![],
        renders: vec![],
        can_undo: false,
        can_redo: false,
    }
}
