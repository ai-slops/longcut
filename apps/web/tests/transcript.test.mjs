import assert from "node:assert/strict"
import test from "node:test"
import {
  cueSelection,
  TranscriptPageSchema,
  transcriptProvenance,
  transcriptRequestMatches,
  wordSelection,
} from "../src/transcript-domain.ts"

const page = {
  id: "transcript-one",
  source_id: "source-one",
  filename: "한국어.srt",
  created_at: "2026-10-03T10:00:00Z",
  frame_rate: 30,
  cue_count: 101,
  origin: "subtitle_import",
  cues: [{ id: "cue-one", start_frame: "30", end_frame: "75", text: "첫 번째 자막" }],
  next_cursor: "opaque-cursor",
}

test("parses Korean transcript pages and preserves opaque pagination cursors", () => {
  // Given an exact transcript response from the API.
  // When it crosses the browser boundary.
  const parsed = TranscriptPageSchema.parse(page)
  // Then Korean text, frame strings, and the cursor remain intact.
  assert.deepEqual(parsed.cues[0], page.cues[0])
  assert.equal(parsed.next_cursor, "opaque-cursor")
})

test("rejects transcript responses with an invented origin", () => {
  // Given a response that does not match the closed wire contract.
  // When it crosses the browser boundary.
  const parsed = TranscriptPageSchema.safeParse({ ...page, origin: "automatic_asr" })
  // Then schema drift is exposed rather than rendered as imported text.
  assert.equal(parsed.success, false)
})

test("parses local ASR metadata without changing subtitle import metadata", () => {
  // Given a published transcript from the local Whisper runtime.
  const localAsrPage = {
    ...page,
    origin: "local_asr",
    asr: {
      engine: "whisper.cpp",
      model: "/models/ggml-small.bin",
      requested_language: "auto",
      result_language: "ko",
    },
  }
  // When both transcript origins cross the browser boundary.
  const parsedAsr = TranscriptPageSchema.parse(localAsrPage)
  const parsedImport = TranscriptPageSchema.parse(page)
  // Then ASR remains explicitly identified and manual imports remain unchanged.
  assert.equal(parsedAsr.origin, "local_asr")
  assert.equal(parsedAsr.asr.requested_language, "auto")
  assert.equal(parsedImport.origin, "subtitle_import")
  assert.equal("asr" in parsedImport, false)
})

test("describes local ASR provenance as machine-generated and unverified", () => {
  // Given a parsed local ASR transcript with automatic language detection.
  const parsed = TranscriptPageSchema.parse({
    ...page,
    origin: "local_asr",
    asr: {
      engine: "whisper.cpp",
      model: "/models/ggml-small.bin",
      requested_language: "auto",
      result_language: "ko",
    },
  })
  // When display provenance is derived from the closed transcript variant.
  const provenance = transcriptProvenance(parsed)
  // Then the UI receives exact engine/model/language data and no verified-human state.
  assert.deepEqual(provenance, {
    kind: "local_asr",
    engine: "whisper.cpp",
    model: "/models/ggml-small.bin",
    requestedLanguage: "auto",
    resultLanguage: "ko",
    verification: "unverified",
  })
})

test("maps a cue to the independent Source seek and range", () => {
  // Given a half-open transcript cue.
  // When it is selected in the transcript list.
  const selection = cueSelection(page.cues[0], page.id)
  // Then Source seek, In, and Out use its exact normalized frames.
  assert.deepEqual(selection, {
    sourceFrame: 30,
    inFrame: 30,
    outFrame: 75,
    evidence: { transcript_id: "transcript-one", cue_ids: ["cue-one"], word_ids: [] },
    previewText: "첫 번째 자막",
  })
})

test("rejects a late transcript response after a newer request starts", () => {
  // Given an older request and a newer request for either the same or another Source.
  // When the response ownership is checked.
  const matches = transcriptRequestMatches(2, 1)
  // Then the stale response cannot replace the current transcript state.
  assert.equal(matches, false)
})

test("rejects a late transcript response after Source deselection invalidates it", () => {
  // Given a request version invalidated without starting a replacement request.
  // When the late response ownership is checked.
  const matches = transcriptRequestMatches(4, 3)
  // Then clearing the selected Source cannot restore its transcript.
  assert.equal(matches, false)
})

test("parses ASR word timing and its explicit model-estimated metadata", () => {
  // Given a current local-ASR transcript response containing word units.
  const response = {
    ...page,
    origin: "local_asr",
    asr: {
      engine: "whisper.cpp",
      model: "/models/ggml-small.bin",
      requested_language: "ko",
      result_language: "ko",
      chunk_seconds: 600,
      overlap_seconds: 2,
      ownership_policy: "half_open_midpoint",
      timing_quality: "model_estimated",
    },
    cues: [
      {
        ...page.cues[0],
        words: [
          { id: "word-one", start_frame: "30", end_frame: "42", text: "첫" },
          { id: "word-punctuation", start_frame: "42", end_frame: "42", text: "." },
        ],
      },
    ],
  }
  // When it crosses the browser boundary.
  const parsed = TranscriptPageSchema.parse(response)
  // Then timing metadata and equal-frame punctuation remain exact strings.
  assert.equal(parsed.asr.timing_quality, "model_estimated")
  assert.deepEqual(parsed.cues[0]?.words?.[1], response.cues[0].words[1])
})

test("maps only positive-duration ASR words to the shared Source selection", () => {
  // Given a selectable spoken word and preserved zero-duration punctuation.
  const spoken = { id: "spoken", start_frame: "30", end_frame: "42", text: "말" }
  const punctuation = { id: "punctuation", start_frame: "42", end_frame: "42", text: "." }
  // When each word requests a Source selection.
  const spokenSelection = wordSelection(spoken, page.id)
  const punctuationSelection = wordSelection(punctuation, page.id)
  // Then only the positive interval seeks Source and sets In/Out.
  assert.deepEqual(spokenSelection, {
    sourceFrame: 30,
    inFrame: 30,
    outFrame: 42,
    evidence: { transcript_id: "transcript-one", cue_ids: [], word_ids: ["spoken"] },
    previewText: "말",
  })
  assert.equal(punctuationSelection, null)
})

test("does not accept fabricated word timing on subtitle imports", () => {
  // Given a manual subtitle payload carrying ASR-only word units.
  const response = {
    ...page,
    cues: [
      {
        ...page.cues[0],
        words: [{ id: "fake", start_frame: "30", end_frame: "75", text: "fake" }],
      },
    ],
  }
  // When the subtitle-import variant crosses the browser boundary.
  const parsed = TranscriptPageSchema.safeParse(response)
  // Then the contract rejects timing that the importer cannot declare.
  assert.equal(parsed.success, false)
})
