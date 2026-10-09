import assert from "node:assert/strict"
import test from "node:test"
import { adoptProject, ProjectRequestCoordinator } from "../src/project-request.ts"

const project = (id, revision) => ({ id, revision })

test("a late project response cannot replace a newer navigation owner", () => {
  // Given request A followed by navigation request B.
  const coordinator = new ProjectRequestCoordinator()
  const requestA = coordinator.begin("project-a")
  const requestB = coordinator.begin("project-b")
  // When A resolves after B owns the surface.
  // Then A is stale while B retains ownership.
  assert.equal(coordinator.release(requestA), false)
  assert.equal(coordinator.owns(requestA), false)
  assert.equal(coordinator.owns(requestB), true)
})

test("a stale finally cannot release a newer busy owner", () => {
  // Given an invalidated command followed by a new project request.
  const coordinator = new ProjectRequestCoordinator()
  const oldCommand = coordinator.begin("project-a")
  coordinator.invalidate()
  const newOpen = coordinator.begin("project-b")
  // When the old command reaches finally.
  // Then it cannot release the new busy owner.
  assert.equal(coordinator.release(oldCommand), false)
  assert.equal(coordinator.release(newOpen), true)
})

test("project adoption requires matching identity and a non-decreasing bigint revision", () => {
  // Given an open project with a revision beyond Number's safe integer range.
  const current = project("project-a", "9007199254740993")
  // When stale or wrong-project refreshes arrive.
  // Then neither response is adopted, while a newer exact revision is.
  assert.equal(adoptProject(current, project("project-a", "9007199254740992")), current)
  assert.equal(adoptProject(current, project("project-b", "9007199254740994")), current)
  assert.deepEqual(
    adoptProject(current, project("project-a", "9007199254740994")),
    project("project-a", "9007199254740994"),
  )
  assert.equal(adoptProject(null, project("project-a", "1")), null)
})
