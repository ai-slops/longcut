mod moments_support;

use longcut_server::model::{
    Command, CommandRequest, MomentEvidenceSelection, MomentNarrativeRole,
};
use moments_support::{fixture, transcript};

async fn store_words(
    fixture: &moments_support::Fixture,
    transcript_id: &str,
    cue_id: &str,
    words: serde_json::Value,
) {
    let parsed: Vec<longcut_server::transcript::Word> =
        serde_json::from_value(words.clone()).expect("parse word fixture");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture._root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("open fixture database");
    sqlx::query("UPDATE transcript_cues SET words=? WHERE id=?")
        .bind(words.to_string())
        .bind(cue_id)
        .execute(&pool)
        .await
        .expect("store compatibility words");
    for (ordinal, word) in parsed.iter().enumerate() {
        sqlx::query("INSERT INTO transcript_words(transcript_id,cue_id,word_id,ordinal,start_frame,end_frame,text) VALUES(?,?,?,?,?,?,?)")
            .bind(transcript_id)
            .bind(cue_id)
            .bind(&word.id)
            .bind(i64::try_from(ordinal).expect("word ordinal"))
            .bind(&word.start_frame)
            .bind(&word.end_frame)
            .bind(&word.text)
            .execute(&pool)
            .await
            .expect("store indexed word");
    }
    pool.close().await;
}

#[tokio::test]
async fn word_ids_freeze_only_the_selected_verbatim_words() {
    // Given
    let fixture = fixture().await;
    let transcript = transcript(&fixture).await;
    let words = serde_json::json!([
        {"id":"word-first","start_frame":"30","end_frame":"40","text":"First"},
        {"id":"word-exact","start_frame":"40","end_frame":"50","text":"exact"},
        {"id":"word-quote","start_frame":"50","end_frame":"60","text":"quote"}
    ]);
    store_words(&fixture, &transcript.id, &transcript.cues[0].id, words).await;
    // When
    let project = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: Command::CreateMoment {
                    source_id: "source".into(),
                    start_frame: "40".into(),
                    end_frame: "60".into(),
                    label: "Words".into(),
                    note: String::new(),
                    narrative_role: MomentNarrativeRole::Other,
                    evidence: Some(MomentEvidenceSelection {
                        transcript_id: transcript.id,
                        cue_ids: vec![],
                        word_ids: vec!["word-exact".into(), "word-quote".into()],
                    }),
                },
            },
        )
        .await
        .expect("create word evidence moment");
    // Then
    assert_eq!(project.moments[0].evidence[0].quote, "exact quote");
    assert_eq!(project.moments[0].evidence[0].start_frame, "40");
    assert_eq!(project.moments[0].evidence[0].end_frame, "60");
}

#[tokio::test]
async fn word_evidence_uses_canonical_punctuation_spacing_for_english_and_korean() {
    // Given
    let fixture = fixture().await;
    let transcript = transcript(&fixture).await;
    let words = serde_json::json!([
        {"id":"english","start_frame":"30","end_frame":"35","text":"hello"},
        {"id":"period","start_frame":"35","end_frame":"36","text":"."},
        {"id":"korean","start_frame":"36","end_frame":"45","text":"안녕하세요"},
        {"id":"question","start_frame":"45","end_frame":"46","text":"?"}
    ]);
    store_words(&fixture, &transcript.id, &transcript.cues[0].id, words).await;

    // When
    let project = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: Command::CreateMoment {
                    source_id: "source".into(),
                    start_frame: "30".into(),
                    end_frame: "46".into(),
                    label: "Punctuation".into(),
                    note: String::new(),
                    narrative_role: MomentNarrativeRole::Other,
                    evidence: Some(MomentEvidenceSelection {
                        transcript_id: transcript.id,
                        cue_ids: vec![],
                        word_ids: vec![
                            "english".into(),
                            "period".into(),
                            "korean".into(),
                            "question".into(),
                        ],
                    }),
                },
            },
        )
        .await
        .expect("create punctuation evidence moment");

    // Then
    assert_eq!(project.moments[0].evidence[0].quote, "hello. 안녕하세요?");
}

#[tokio::test]
async fn word_evidence_reads_index_without_scanning_legacy_json() {
    // Given
    let fixture = fixture().await;
    let transcript = transcript(&fixture).await;
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture._root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("open fixture database");
    let cue_id = &transcript.cues[0].id;
    for (ordinal, id, start, end, text) in [
        (0_i64, "indexed-hello", "30", "40", "hello"),
        (1, "indexed-period", "40", "40", "."),
    ] {
        sqlx::query("INSERT INTO transcript_words(transcript_id,cue_id,word_id,ordinal,start_frame,end_frame,text) VALUES(?,?,?,?,?,?,?)")
            .bind(&transcript.id)
            .bind(cue_id)
            .bind(id)
            .bind(ordinal)
            .bind(start)
            .bind(end)
            .bind(text)
            .execute(&pool)
            .await
            .expect("insert indexed word");
    }
    sqlx::query("UPDATE transcript_cues SET words='{malformed' WHERE id=?")
        .bind(cue_id)
        .execute(&pool)
        .await
        .expect("corrupt compatibility JSON");
    pool.close().await;

    // When
    let project = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: Command::CreateMoment {
                    source_id: "source".into(),
                    start_frame: "30".into(),
                    end_frame: "41".into(),
                    label: "Indexed words".into(),
                    note: String::new(),
                    narrative_role: MomentNarrativeRole::Other,
                    evidence: Some(MomentEvidenceSelection {
                        transcript_id: transcript.id,
                        cue_ids: vec![],
                        word_ids: vec!["indexed-period".into(), "indexed-hello".into()],
                    }),
                },
            },
        )
        .await
        .expect("resolve indexed words");

    // Then
    assert_eq!(project.moments[0].evidence[0].quote, "hello.");
    assert_eq!(
        project.moments[0].evidence[0].word_ids,
        ["indexed-hello", "indexed-period"]
    );
}
