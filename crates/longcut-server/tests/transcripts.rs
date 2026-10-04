use longcut_server::{
    model::*,
    store::Store,
    transcript::{ImportTranscript, Search},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn fixture() -> Result<(tempfile::TempDir, Store, Project), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let store = Store::open(&root.path().join("project.sqlite3")).await?;
    let project = store
        .create(CreateProject {
            title: "transcripts".into(),
            frame_rate: 30,
        })
        .await?;
    let project = store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "original".into(),
                role: SourceRole::Program,
                duration_frames: "300".into(),
                width: 160,
                height: 90,
                has_audio: true,
                media_url: "/api/v1/media/original.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: vec![],
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await?;
    Ok((root, store, project))
}

#[tokio::test]
async fn imported_korean_transcript_is_searchable_without_changing_edit_revision() -> TestResult {
    // Given
    let (_root, store, project) = fixture().await?;
    let imported = store.import_transcript(&project.id, "source", ImportTranscript { filename: "자막.srt".into(), text: "1\n00:00:00,000 --> 00:00:01,000\n첫 번째 실패 원인\n\n2\n00:00:02,000 --> 00:00:03,000\n전략 변경 후 성공".into() }).await?;
    // When
    let phrase = store
        .transcript(
            &project.id,
            "source",
            Search {
                query: Some("실패 원인".into()),
                cursor: None,
            },
        )
        .await?
        .ok_or("transcript")?;
    let short = store
        .transcript(
            &project.id,
            "source",
            Search {
                query: Some("성공".into()),
                cursor: None,
            },
        )
        .await?
        .ok_or("transcript")?;
    // Then
    assert_eq!(imported.cue_count, 2);
    assert_eq!(phrase.cues.len(), 1);
    assert_eq!(phrase.cues[0].start_frame, "0");
    assert_eq!(short.cues[0].start_frame, "60");
    assert_eq!(store.get(&project.id).await?.revision, project.revision);
    Ok(())
}

#[tokio::test]
async fn paging_is_pinned_to_an_immutable_transcript_version() -> TestResult {
    // Given
    let (_root, store, project) = fixture().await?;
    let text = (0..101)
        .map(|index| format!("{index}\n00:00:00,000 --> 00:00:01,000\n원문 {index}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let old = store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "old.srt".into(),
                text,
            },
        )
        .await?;
    store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "new.srt".into(),
                text: "1\n00:00:00,000 --> 00:00:01,000\n새 버전".into(),
            },
        )
        .await?;
    // When
    let page = store
        .transcript(
            &project.id,
            "source",
            Search {
                query: None,
                cursor: old.next_cursor,
            },
        )
        .await?
        .ok_or("old page")?;
    // Then
    assert_eq!(page.id, old.id);
    assert_eq!(page.cues.len(), 1);
    assert_eq!(page.cues[0].text, "원문 100");
    assert!(page.next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn invalid_cues_do_not_replace_previous_transcript() -> TestResult {
    // Given
    let (_root, store, project) = fixture().await?;
    let old = store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "old.srt".into(),
                text: "1\n00:00:00,000 --> 00:00:01,000\n유지".into(),
            },
        )
        .await?;
    // When
    let result = store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "bad.srt".into(),
                text: "1\n00:00:00,000 --> 00:01:00,000\n범위 초과".into(),
            },
        )
        .await;
    // Then
    assert!(result.is_err());
    assert_eq!(
        store
            .transcript(&project.id, "source", Search::default())
            .await?
            .ok_or("old")?
            .id,
        old.id
    );
    assert!(
        store
            .transcript(&project.id, "other", Search::default())
            .await
            .is_err()
    );
    Ok(())
}
