import assert from "node:assert/strict"
import test from "node:test"
import {
  cueSelection,
  TranscriptPageSchema,
  transcriptRequestMatches,
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

test("maps a cue to the independent Source seek and range", () => {
  // Given a half-open transcript cue.
  // When it is selected in the transcript list.
  const selection = cueSelection(page.cues[0])
  // Then Source seek, In, and Out use its exact normalized frames.
  assert.deepEqual(selection, { sourceFrame: 30, inFrame: 30, outFrame: 75 })
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
