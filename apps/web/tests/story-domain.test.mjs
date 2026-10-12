import assert from "node:assert/strict"
import test from "node:test"
import { SceneSchema, StorySchema } from "../src/domain.ts"
import {
  assignmentAvailability,
  budgetSummary,
  createStoryCommand,
  editableStoryDraft,
  linkableMomentHeads,
  reconcileStoryDraft,
  storyLimits,
  validateStoryText,
  validateTargetFrames,
} from "../src/story-domain.ts"
import {
  storyVersionIdentity,
  storyVersionResponseMatchesIdentity,
} from "../src/story-version-request.ts"

test("legacy Story and Scene fields receive safe defaults", () => {
  // Given legacy wire objects without Story or beat assignment fields.
  const story = StorySchema.parse(undefined)
  const scene = SceneSchema.parse({
    id: "scene",
    title: "Opening",
    purpose: "",
    source_id: "source",
    in_frame: "0",
    out_frame: "30",
    camera: null,
    caption: "",
    locked: false,
    manual: true,
  })
  // When boundary defaults are applied, then old projects remain editable.
  assert.deepEqual(story, { version: "0", chapters: [] })
  assert.equal(scene.beat_id, null)
})

test("Story commands pin the opened Story version", () => {
  // Given an update opened at Story version 12.
  const command = createStoryCommand("12", {
    type: "update_beat",
    beat_id: "beat",
    title: "Cold open",
    purpose: "Lead with the result",
    target_frames: "900",
  })
  // When the command is built, then the documented nested envelope is exact.
  assert.deepEqual(command, {
    type: "story",
    action: {
      type: "update_beat",
      expected_story_version: "12",
      beat_id: "beat",
      title: "Cold open",
      purpose: "Lead with the result",
      target_frames: "900",
    },
  })
})

test("budget math preserves integers beyond Number precision", () => {
  // Given an advisory target and scene total above Number.MAX_SAFE_INTEGER.
  const result = budgetSummary("90071992547409931234", [
    { in_frame: "0", out_frame: "90071992547409931235" },
    { in_frame: "5", out_frame: "9" },
  ])
  // When the delta is computed, then BigInt exactness is retained.
  assert.deepEqual(result, {
    target: "90071992547409931234",
    actual: "90071992547409931239",
    delta: "+5",
  })
})

test("target frames accept only optional positive u32 strings", () => {
  // Given valid and invalid form values.
  const values = ["", "1", "2147483647", "0", "01", "2147483648", "1.5", "1e3"]
  // When validated, then only canonical values in the API range pass.
  assert.deepEqual(values.map(validateTargetFrames), [
    null,
    "1",
    "2147483647",
    false,
    false,
    false,
    false,
    false,
  ])
})

test("Story text and count constraints match the wire contract", () => {
  // Given multibyte labels and published entity caps.
  const title = validateStoryText("界".repeat(67), "title")
  const purpose = validateStoryText("界".repeat(667), "purpose")
  // When UTF-8 bytes are counted, then limits are not measured as UTF-16 length.
  assert.equal(title.valid, false)
  assert.equal(purpose.valid, false)
  assert.deepEqual(storyLimits, { chapters: 64, beats: 512, beatsPerChapter: 128, refsPerBeat: 64 })
})

test("only current reviewed non-archived Moment heads are newly linkable", () => {
  // Given current heads and an already-pinned historical reference.
  const moments = [
    { id: "ready", version: "4", review_status: "reviewed", archived: false },
    { id: "draft", version: "2", review_status: "draft", archived: false },
    { id: "archived", version: "7", review_status: "reviewed", archived: true },
  ]
  // When eligibility is computed, then the old pin survives but cannot be added as latest.
  assert.deepEqual(linkableMomentHeads(moments, [{ moment_id: "archived", version: "3" }]), {
    available: [{ moment_id: "ready", version: "4" }],
    pinned: [{ moment_id: "archived", version: "3" }],
  })
})

test("locked scenes explain why a changed assignment is unavailable", () => {
  // Given a locked scene already assigned to one beat.
  const changed = assignmentAvailability({ locked: true, beat_id: "beat-a" }, "beat-b")
  const unchanged = assignmentAvailability({ locked: true, beat_id: "beat-a" }, "beat-a")
  // When choosing assignments, then only the server-accepted no-op stays enabled.
  assert.deepEqual(changed, {
    enabled: false,
    reason: "Unlock this scene before changing its beat.",
  })
  assert.deepEqual(unchanged, { enabled: true, reason: null })
})

test("opened Story forms preserve drafts and become stale after an external version change", () => {
  // Given a beat form opened at version 8 with a local draft.
  const draft = editableStoryDraft("8", "8", { title: "Draft title", purpose: "Local notes" })
  const stale = editableStoryDraft("8", "9", draft.fields)
  // When a newer outline arrives, then the draft is retained and saving is disabled.
  assert.equal(draft.stale, false)
  assert.deepEqual(stale, { stale: true, fields: { title: "Draft title", purpose: "Local notes" } })
})

test("Story forms reconcile pristine, self-saved, and conflicting version changes", () => {
  // Given the same form can receive unrelated, self-save, or conflicting server updates.
  const baseline = { title: "Opening", purpose: "Set the stakes" }
  const pristine = { openedVersion: "8", baseline, fields: baseline }
  const local = {
    openedVersion: "8",
    baseline,
    fields: { title: "Cold open", purpose: "Set the stakes" },
  }

  // When versions advance, then safe updates reconcile and only a true dirty conflict stays stale.
  assert.deepEqual(reconcileStoryDraft(pristine, "9", baseline), {
    openedVersion: "9",
    baseline,
    fields: baseline,
  })
  assert.deepEqual(reconcileStoryDraft(local, "9", local.fields), {
    openedVersion: "9",
    baseline: local.fields,
    fields: local.fields,
  })
  assert.equal(
    reconcileStoryDraft(local, "9", { title: "Server edit", purpose: "Set the stakes" }),
    local,
  )
})

test("Story snapshot preview rejects a response for another project or version", () => {
  // Given a preview request pinned to one project and Story version.
  const identity = storyVersionIdentity("project-a", "12")
  // When a response crosses the boundary, then every identity field must match.
  assert.equal(
    storyVersionResponseMatchesIdentity(identity, {
      project_id: "project-b",
      version: "12",
      snapshot: { version: "12", chapters: [] },
    }),
    false,
  )
})
