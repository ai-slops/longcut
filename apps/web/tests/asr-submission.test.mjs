import assert from "node:assert/strict"
import test from "node:test"
import { AsrJobSchema } from "../src/asr-domain.ts"
import {
  AsrActionContractError,
  AsrSubmissionCoordinator,
  canSubmitAsrResume,
} from "../src/asr-submission.ts"

const scope = { projectId: "project-one", sourceId: "source-one" }
const failedJob = AsrJobSchema.parse({
  id: "job-one",
  project_id: scope.projectId,
  source_id: scope.sourceId,
  status: "failed",
  progress: { completed_chunks: 2, total_chunks: 5 },
  language: "en",
  engine: "whisper.cpp",
  model: "/models/local.bin",
  created_at: "2026-10-09T10:00:00Z",
  updated_at: "2026-10-09T10:01:00Z",
  transcript_id: null,
  error: "stopped",
  resume_eligible: true,
})

function deferred() {
  let resolve
  let reject
  const promise = new Promise((onResolve, onReject) => {
    resolve = onResolve
    reject = onReject
  })
  return { promise, resolve, reject }
}

test("pending Start synchronously blocks Resume until the delayed action settles", async () => {
  // Given a delayed Start request that already owns the coordinator.
  const coordinator = new AsrSubmissionCoordinator()
  const request = deferred()
  const start = coordinator.begin(scope, "start")
  assert.ok(start)
  const outcome = coordinator.settle(start, request.promise)
  // When Resume is attempted before React could render a disabled button.
  const resume = coordinator.begin(scope, "resume")
  // Then no second action can replace or abort the pending Start.
  assert.equal(resume, null)
  request.resolve({ ...failedJob, status: "queued", error: null, resume_eligible: false })
  assert.equal((await outcome).kind, "accepted")
})

test("mismatched action responses are rejected instead of replacing the current job", async () => {
  // Given cancellation owned by one project, Source, and job ID.
  const coordinator = new AsrSubmissionCoordinator()
  const ticket = coordinator.begin(scope, "cancel")
  assert.ok(ticket)
  // When the server response belongs to another Source.
  const outcome = await coordinator.settle(
    ticket,
    Promise.resolve({ ...failedJob, source_id: "source-two" }),
    failedJob.id,
  )
  // Then the response becomes a controlled contract failure.
  assert.equal(outcome.kind, "failed")
  if (outcome.kind === "failed") assert.ok(outcome.reason instanceof AsrActionContractError)
})

test("Cancel rejects a response with a different job ID", async () => {
  // Given cancellation of one exact recovered job.
  const coordinator = new AsrSubmissionCoordinator()
  const ticket = coordinator.begin(scope, "cancel")
  assert.ok(ticket)
  // When the route returns another job from the same Source.
  const outcome = await coordinator.settle(
    ticket,
    Promise.resolve({ ...failedJob, id: "job-two" }),
    failedJob.id,
  )
  // Then identity validation prevents replacing the recovered job.
  assert.equal(outcome.kind, "failed")
  if (outcome.kind === "failed") assert.ok(outcome.reason instanceof AsrActionContractError)
})

test("a resume conflict retains the terminal job and releases submission ownership", async () => {
  // Given an eligible failed job and an owned Resume request.
  const coordinator = new AsrSubmissionCoordinator()
  const ticket = coordinator.begin(scope, "resume")
  assert.ok(ticket)
  let currentJob = failedJob
  // When the request rejects with a conflict.
  const outcome = await coordinator.settle(
    ticket,
    Promise.reject(new Error("409 conflict")),
    failedJob.id,
  )
  if (outcome.kind === "accepted") currentJob = outcome.job
  // Then the failed job remains available and another action may start.
  assert.equal(currentJob, failedJob)
  assert.equal(outcome.kind, "failed")
  assert.equal(outcome.released, true)
  assert.ok(coordinator.begin(scope, "resume"))
})

test("a Source change invalidates the old action without its completion clearing the new guard", async () => {
  // Given a delayed action for the previous Source.
  const coordinator = new AsrSubmissionCoordinator()
  const oldRequest = deferred()
  const oldTicket = coordinator.begin(scope, "start")
  assert.ok(oldTicket)
  const oldOutcome = coordinator.settle(oldTicket, oldRequest.promise)
  // When Source ownership changes and a new action starts.
  coordinator.invalidate()
  const nextTicket = coordinator.begin({ ...scope, sourceId: "source-two" }, "start")
  assert.ok(nextTicket)
  oldRequest.resolve({ ...failedJob, status: "queued" })
  // Then the late old result is stale and the new guard remains owned.
  assert.deepEqual(await oldOutcome, { kind: "stale", released: false })
  assert.equal(coordinator.activeAction, "start")
})

test("an unavailable runtime or active submission prevents Resume", () => {
  // Given a resume-eligible terminal job.
  // When runtime and ownership states differ.
  const outcomes = [
    canSubmitAsrResume(failedJob, false, false),
    canSubmitAsrResume(failedJob, true, true),
    canSubmitAsrResume(failedJob, true, false),
  ]
  // Then only an available idle runtime permits Resume.
  assert.deepEqual(outcomes, [false, false, true])
})
