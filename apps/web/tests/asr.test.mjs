import assert from "node:assert/strict"
import test from "node:test"
import {
  AsrJobSchema,
  AsrRuntimeStatusSchema,
  asrJobBelongsToSource,
  asrJobStorageKey,
  asrRequestMatches,
  canPollAsrJob,
  canResumeAsrJob,
  canRetryAsrPoll,
  isActiveAsrJob,
} from "../src/asr-domain.ts"

const job = {
  id: "job-one",
  project_id: "project-one",
  source_id: "source-one",
  status: "running",
  progress: { completed_chunks: 2, total_chunks: 5 },
  language: "auto",
  engine: "whisper.cpp",
  model: "/models/ggml-small.bin",
  created_at: "2026-10-04T10:00:00Z",
  updated_at: "2026-10-04T10:01:00Z",
  transcript_id: null,
  error: null,
}

test("parses an available local ASR runtime contract", () => {
  // Given the exact runtime response advertised by the backend.
  const status = {
    configured: true,
    available: true,
    cli_available: true,
    model_available: true,
    ffmpeg_available: true,
    missing: [],
    engine: "whisper.cpp",
    model: "/models/ggml-small.bin",
    default_language: "ko",
    supported_languages: ["ko", "en", "auto"],
    chunk_seconds: 600,
    max_concurrency: 1,
  }
  // When it crosses the browser boundary.
  const parsed = AsrRuntimeStatusSchema.parse(status)
  // Then availability and the closed language list remain intact.
  assert.equal(parsed.available, true)
  assert.deepEqual(parsed.supported_languages, ["ko", "en", "auto"])
})

test("rejects ASR progress that exceeds its total chunks", () => {
  // Given impossible persisted job progress.
  // When the response crosses the browser boundary.
  const parsed = AsrJobSchema.safeParse({
    ...job,
    progress: { completed_chunks: 6, total_chunks: 5 },
  })
  // Then schema drift is rejected before the UI renders it.
  assert.equal(parsed.success, false)
})

test("treats legacy ASR jobs without resume metadata as ineligible", () => {
  // Given an older persisted job fixture without resume_eligible.
  // When it crosses the browser boundary.
  const parsed = AsrJobSchema.parse({ ...job, status: "failed" })
  // Then the UI cannot present a misleading resume action.
  assert.equal(parsed.resume_eligible, false)
})

test("allows explicit resume only for eligible failed or cancelled jobs", () => {
  // Given jobs that advertise retained resume state across terminal and active statuses.
  const failed = AsrJobSchema.parse({ ...job, status: "failed", resume_eligible: true })
  const cancelled = AsrJobSchema.parse({ ...job, status: "cancelled", resume_eligible: true })
  const running = AsrJobSchema.parse({ ...job, resume_eligible: true })
  // When resume eligibility is evaluated.
  const outcomes = [canResumeAsrJob(failed), canResumeAsrJob(cancelled), canResumeAsrJob(running)]
  // Then only eligible resumable terminal states expose the action.
  assert.deepEqual(outcomes, [true, true, false])
})

test("scopes recovered ASR jobs to the selected project and source", () => {
  // Given a persisted job from the previously selected Source.
  const parsed = AsrJobSchema.parse(job)
  // When ownership is checked after a Source switch.
  const matches = asrJobBelongsToSource(parsed, "project-one", "source-two")
  // Then the stale job cannot appear in the new Source controls.
  assert.equal(matches, false)
})

test("uses an isolated persisted job key for every project and source", () => {
  // Given two Sources in the same project.
  // When their recovery keys are created.
  const first = asrJobStorageKey("project-one", "source-one")
  const second = asrJobStorageKey("project-one", "source-two")
  // Then one Source cannot recover the other's job.
  assert.notEqual(first, second)
})

test("polls only queued and running ASR jobs", () => {
  // Given active and terminal job states.
  // When polling eligibility is evaluated.
  const active = isActiveAsrJob(AsrJobSchema.parse(job))
  const terminal = isActiveAsrJob(AsrJobSchema.parse({ ...job, status: "ready" }))
  // Then terminal jobs do not keep background polling alive.
  assert.equal(active, true)
  assert.equal(terminal, false)
})

test("rejects an in-flight poll after cancellation claims a new request generation", () => {
  // Given a poll owned by the previous request generation.
  const pollVersion = 7
  // When cancellation advances ownership before making its request.
  const cancellationVersion = pollVersion + 1
  // Then the late poll cannot replace the cancellation response.
  assert.equal(asrRequestMatches(cancellationVersion, pollVersion), false)
})

test("offers poll retry only after an active job status request fails", () => {
  // Given an active job with and without a failed status request.
  const activeJob = AsrJobSchema.parse(job)
  // When retry eligibility is evaluated.
  const beforeFailure = canRetryAsrPoll(activeJob, false)
  const afterFailure = canRetryAsrPoll(activeJob, true)
  const terminalFailure = canRetryAsrPoll(AsrJobSchema.parse({ ...job, status: "cancelled" }), true)
  // Then only the failed active poll exposes recovery.
  assert.equal(beforeFailure, false)
  assert.equal(afterFailure, true)
  assert.equal(terminalFailure, false)
})

test("resumes polling after a failed cancellation finishes", () => {
  // Given an active job while cancellation owns the request lifecycle.
  const activeJob = AsrJobSchema.parse(job)
  // When cancellation is submitting and then fails without changing the job.
  const duringCancel = canPollAsrJob(activeJob, false, true)
  const afterFailedCancel = canPollAsrJob(activeJob, false, false)
  // Then polling stays paused during cancellation and resumes afterward.
  assert.equal(duringCancel, false)
  assert.equal(afterFailedCancel, true)
})
