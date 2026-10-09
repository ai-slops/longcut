import assert from "node:assert/strict"
import test from "node:test"
import {
  MomentVersionRequestCoordinator,
  momentVersionIdentity,
  snapshotMatchesIdentity,
  versionResponseMatchesIdentity,
} from "../src/moment-version-request.ts"

test("a late historical response cannot display under a new version identity", () => {
  // Given a v7 request followed by the same Moment at v10.
  const coordinator = new MomentVersionRequestCoordinator()
  const v7 = coordinator.begin(momentVersionIdentity("project", "moment", "7"))
  const v10 = coordinator.begin(momentVersionIdentity("project", "moment", "10"))
  // When v7 resolves after v10 owns the preview.
  // Then only the v10 ticket remains displayable.
  assert.equal(coordinator.owns(v7), false)
  assert.equal(coordinator.owns(v10), true)
  assert.equal(v7.signal.aborted, true)
})

test("historical response envelope must match the complete requested identity", () => {
  // Given a request for one project's Moment v10.
  const identity = momentVersionIdentity("project", "moment", "10")
  const response = {
    project_id: "project",
    moment_id: "moment",
    version: "10",
    snapshot: { id: "moment", version: "10" },
  }
  // When any envelope or snapshot identity field drifts.
  // Then only the fully matching response is accepted.
  assert.equal(versionResponseMatchesIdentity(identity, response), true)
  assert.equal(
    versionResponseMatchesIdentity(identity, { ...response, project_id: "other" }),
    false,
  )
  assert.equal(versionResponseMatchesIdentity(identity, { ...response, version: "7" }), false)
  assert.equal(
    versionResponseMatchesIdentity(identity, {
      ...response,
      snapshot: { id: "other", version: "10" },
    }),
    false,
  )
})

test("historical snapshot identity must match both Moment and version", () => {
  // Given the requested v10 identity.
  const identity = momentVersionIdentity("project", "moment", "10")
  // When snapshots for another Moment or version arrive.
  // Then neither can be displayed under v10.
  assert.equal(snapshotMatchesIdentity(identity, { id: "moment", version: "10" }), true)
  assert.equal(snapshotMatchesIdentity(identity, { id: "moment", version: "7" }), false)
  assert.equal(snapshotMatchesIdentity(identity, { id: "other", version: "10" }), false)
})
