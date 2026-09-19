import assert from "node:assert/strict"
import test from "node:test"
import { syncDraft } from "../src/draft.ts"

test("refreshes a clean field when undo changes the same entity", () => {
  const state = { entity: "scene", baseline: "Saved title", value: "Saved title" }
  const result = syncDraft(state, { entity: "scene", value: "Original title" })
  assert.equal(result.value, "Original title")
})
test("preserves unsaved input when a poll updates the server value", () => {
  const state = { entity: "scene", baseline: "Original", value: "Unsaved draft" }
  const result = syncDraft(state, { entity: "scene", value: "Server revision" })
  assert.equal(result.value, "Unsaved draft")
  assert.equal(result.baseline, "Server revision")
})
test("resets unsaved input when selecting another entity", () => {
  const state = { entity: "first", baseline: "Original", value: "Unsaved draft" }
  const result = syncDraft(state, { entity: "second", value: "Second title" })
  assert.equal(result.value, "Second title")
})
test("recognizes a saved draft as clean for subsequent undo", () => {
  const state = { entity: "scene", baseline: "Original", value: "Edited" }
  const saved = syncDraft(state, { entity: "scene", value: "Edited" })
  const undone = syncDraft(saved, { entity: "scene", value: "Original" })
  assert.equal(undone.value, "Original")
})
