import assert from "node:assert/strict"
import test from "node:test"
import { RuntimeRequestCoordinator } from "../src/runtime-request.ts"

test("a project or run switch aborts and invalidates the prior runtime request", () => {
  // Given an expanded run followed by a different run identity.
  const coordinator = new RuntimeRequestCoordinator()
  const first = coordinator.begin({ projectId: "project-a", runId: "run-a" })
  const second = coordinator.begin({ projectId: "project-b", runId: "run-b" })
  // When the first response arrives late.
  // Then it is aborted and cannot own the visible runtime details.
  assert.equal(first.signal.aborted, true)
  assert.equal(coordinator.owns(first), false)
  assert.equal(coordinator.owns(second), true)
})

test("collapsing runtime details aborts the request and clears ownership", () => {
  // Given one in-flight runtime details request.
  const coordinator = new RuntimeRequestCoordinator()
  const request = coordinator.begin({ projectId: "project-a", runId: "run-a" })
  // When the disclosure closes.
  coordinator.invalidate()
  // Then no late response or timer owns the surface.
  assert.equal(request.signal.aborted, true)
  assert.equal(coordinator.owns(request), false)
})
