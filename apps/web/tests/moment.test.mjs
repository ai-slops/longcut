import assert from "node:assert/strict"
import test from "node:test"
import { ProjectSchema } from "../src/domain.ts"
import {
  buildMomentUpdate,
  canInsertMoment,
  evidenceForSelection,
  momentArchiveCommand,
  momentQuality,
  momentReviewCommand,
  momentSceneCommand,
  prepareMomentUpdate,
  validateMomentRange,
  validateMomentText,
} from "../src/moment-workflow.ts"

const evidence = {
  transcript_id: "transcript-one",
  cue_ids: ["cue-one"],
  word_ids: [],
  quote: "있는 그대로의 인용문",
  origin: "subtitle_import",
  start_frame: "30",
  end_frame: "75",
  timing_basis: "cues",
}

const moment = {
  id: "moment-one",
  version: "7",
  source_id: "source-one",
  start_frame: "30",
  end_frame: "75",
  label: "회복의 시작",
  note: "",
  narrative_role: "insight",
  origin: "manual",
  review_status: "reviewed",
  archived: false,
  evidence: [evidence],
  provenance: {
    created_command_id: "create-command",
    updated_command_id: "update-command",
    created_revision: "6",
    updated_revision: "7",
    created_at: "2026-10-09T13:00:00Z",
    updated_at: "2026-10-09T13:05:00Z",
  },
}

const visualSource = {
  id: "source-one",
  name: "Interview",
  role: "program",
  duration_frames: "900",
  width: 1920,
  height: 1080,
  has_audio: true,
  media_url: "/media/source-one",
  proxy_url: null,
  thumbnail_url: null,
  waveform: [],
  status: "ready",
  error: null,
}

test("defaults legacy projects and scenes without Moment fields", () => {
  // Given a legacy project response with no moments or scene lineage.
  const legacy = {
    id: "project-one",
    title: "Legacy",
    revision: "1",
    frame_rate: 30,
    updated_at: "2026-10-09T13:00:00Z",
    brief: "",
    sources: [],
    source_sessions: [],
    scenes: [
      {
        id: "scene-one",
        title: "Opening",
        purpose: "",
        source_id: "source-one",
        in_frame: "0",
        out_frame: "30",
        camera: null,
        caption: "",
        locked: false,
        manual: true,
      },
    ],
    feedback: [],
    change_sets: [],
    renders: [],
    can_undo: false,
    can_redo: false,
  }
  // When the wire response is parsed.
  const parsed = ProjectSchema.parse(legacy)
  // Then backward-compatible defaults are materialized.
  assert.deepEqual(parsed.moments, [])
  assert.equal(parsed.scenes[0].moment, null)
})

test("resends immutable evidence IDs when editorial text changes", () => {
  // Given a Moment edit that keeps the exact source and range.
  // When the update command is built from its frozen snapshot.
  const command = buildMomentUpdate(moment, {
    label: "새 제목",
    note: "새 메모",
    narrativeRole: "goal",
  })
  // Then IDs are resent without fabricated quote or timing fields.
  assert.deepEqual(command.evidence, {
    transcript_id: "transcript-one",
    cue_ids: ["cue-one"],
    word_ids: [],
  })
  assert.equal(command.expected_moment_version, "7")
  assert.equal("quote" in command.evidence, false)
})

test("clears evidence when a changed range no longer contains it", () => {
  // Given a range change that excludes the frozen evidence interval.
  // When the update command is built.
  const command = buildMomentUpdate(moment, { startFrame: "40", endFrame: "70" })
  // Then null explicitly clears evidence instead of reusing wrong-source evidence.
  assert.equal(command.evidence, null)
})

test("clears evidence when a widened range drifts from the opened Moment", () => {
  // Given frozen evidence for the exact opened 30–75 Moment range.
  // When an edit widens that range while still containing the quote.
  const command = buildMomentUpdate(moment, { startFrame: "0", endFrame: "90" })
  // Then evidence is cleared because only the exact opened range may retain it.
  assert.equal(command.evidence, null)
})

test("keeps selected evidence only for its exact source and range", () => {
  // Given immutable cue evidence captured for one source interval.
  const selection = {
    sourceId: "source-one",
    inFrame: 30,
    outFrame: 75,
    evidence: { transcript_id: "transcript-one", cue_ids: ["cue-one"], word_ids: [] },
    previewText: "있는 그대로의 인용문",
  }
  // When the source or range changes manually.
  // Then only an exact selection can retain the IDs.
  assert.deepEqual(
    evidenceForSelection({ selection, sourceId: "source-one", inFrame: 30, outFrame: 75 }),
    selection.evidence,
  )
  assert.equal(
    evidenceForSelection({ selection, sourceId: "source-two", inFrame: 30, outFrame: 75 }),
    null,
  )
  assert.equal(
    evidenceForSelection({ selection, sourceId: "source-one", inFrame: 31, outFrame: 75 }),
    null,
  )
})

test("gates scene insertion by review, archive, and visual media", () => {
  // Given reviewed and draft Moments plus visual and audio-only Sources.
  const audioSource = { ...visualSource, role: "audio", width: 0, height: 0 }
  // When insertion eligibility is derived.
  // Then only reviewed, active, visual Moments can become Scenes.
  assert.equal(canInsertMoment(moment, visualSource), true)
  assert.equal(canInsertMoment({ ...moment, review_status: "draft" }, visualSource), false)
  assert.equal(canInsertMoment({ ...moment, archived: true }, visualSource), false)
  assert.equal(canInsertMoment(moment, audioSource), false)
})

test("uses UTF-8 bytes for label and note limits", () => {
  // Given multibyte labels at and beyond the 200-byte boundary.
  const exact = `${"界".repeat(66)}ab`
  const over = `${exact}界`
  // When text validity is calculated.
  // Then browser validation matches backend byte limits.
  assert.deepEqual(validateMomentText(exact, ""), { labelBytes: 200, noteBytes: 0, valid: true })
  assert.equal(validateMomentText(over, "").valid, false)
  assert.equal(validateMomentText("ok", "界".repeat(1334)).valid, false)
})

test("rejects labels that become empty after backend-equivalent trimming", () => {
  // Given labels containing only ASCII or Unicode whitespace.
  // When text validity is calculated.
  // Then neither label is accepted even though it has UTF-8 bytes.
  assert.equal(validateMomentText("   ", "").valid, false)
  assert.equal(validateMomentText("\n\t　", "").valid, false)
})

test("classifies non-positive and out-of-source Moment ranges", () => {
  // Given candidate ranges and a 900-frame Source.
  // When range validity is calculated.
  // Then each invalid interval has a machine-readable reason.
  assert.deepEqual(
    validateMomentRange({ startFrame: "30", endFrame: "30", durationFrames: "900" }),
    {
      valid: false,
      reason: "non_positive",
    },
  )
  assert.deepEqual(
    validateMomentRange({ startFrame: "30", endFrame: "901", durationFrames: "900" }),
    {
      valid: false,
      reason: "outside_source",
    },
  )
  assert.deepEqual(
    validateMomentRange({ startFrame: "30", endFrame: "75", durationFrames: "900" }),
    {
      valid: true,
      reason: null,
    },
  )
})

test("blocks a stale edit without rebasing source, range, or evidence", () => {
  // Given an editor opened at v7 whose draft changes the Source and range.
  const draft = {
    sourceId: "source-two",
    startFrame: "10",
    endFrame: "20",
    label: "unsaved draft",
    note: "still local",
    narrativeRole: "attempt",
  }
  // When the current head advances externally to v8.
  const result = prepareMomentUpdate({ base: moment, currentVersion: "8", draft })
  // Then submission is blocked and no command is silently rebuilt from v8.
  assert.deepEqual(result, { kind: "stale", baseVersion: "7", currentVersion: "8" })
})

test("keeps malformed edit frames visible without building a command", () => {
  // Given raw number-input values that BigInt cannot parse safely.
  const invalidFrames = ["1.5", "1e3", ""]
  // When each draft is prepared during render.
  // Then invalid drafts produce no command and never throw.
  for (const startFrame of invalidFrames)
    assert.deepEqual(
      prepareMomentUpdate({
        base: moment,
        currentVersion: "7",
        draft: {
          sourceId: "source-one",
          startFrame,
          endFrame: "75",
          label: "local draft",
          note: "",
          narrativeRole: "attempt",
        },
      }),
      { kind: "invalid", reason: "invalid_format" },
    )
})

test("builds an edit only from the immutable version opened by the editor", () => {
  // Given a v7 editor draft that changes the range beyond its frozen evidence.
  const draft = {
    sourceId: "source-one",
    startFrame: "40",
    endFrame: "70",
    label: "local draft",
    note: "",
    narrativeRole: "attempt",
  }
  // When the displayed head is still v7.
  const result = prepareMomentUpdate({ base: moment, currentVersion: "7", draft })
  // Then the command remains pinned to v7 and explicitly clears incompatible evidence.
  assert.equal(result.kind, "ready")
  if (result.kind === "ready") {
    assert.equal(result.command.expected_moment_version, "7")
    assert.equal(result.command.source_id, "source-one")
    assert.equal(result.command.start_frame, "40")
    assert.equal(result.command.end_frame, "70")
    assert.equal(result.command.evidence, null)
  }
})

test("models review quality as machine-readable editorial state", () => {
  // Given reviewed local-ASR evidence.
  const asrMoment = { ...moment, evidence: [{ ...evidence, origin: "local_asr" }] }
  // When quality labels are derived.
  const quality = momentQuality(asrMoment)
  // Then editorial review never becomes ASR or synchronization verification.
  assert.deepEqual(quality, {
    reviewStatus: "reviewed",
    verification: "editorial_only",
    transcriptOrigin: "local_asr",
    transcriptVerification: "unverified",
  })
})

test("pins entity versions in every Moment lifecycle command", () => {
  // Given the currently displayed Moment head.
  // When review, archive, and Scene insertion commands are built.
  const review = momentReviewCommand(moment, "reviewed")
  const archive = momentArchiveCommand(moment)
  const insert = momentSceneCommand(moment)
  // Then every payload carries the exact displayed Moment version.
  assert.equal(review.expected_moment_version, "7")
  assert.equal(archive.expected_moment_version, "7")
  assert.equal(insert.expected_moment_version, "7")
  assert.equal(insert.title, "회복의 시작")
})
