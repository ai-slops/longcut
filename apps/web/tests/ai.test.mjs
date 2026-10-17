import assert from "node:assert/strict"
import test from "node:test"
import {
  AiRequestSchema,
  AiRunSchema,
  CodexStatusSchema,
  ProviderStatusesSchema,
  RunRuntimeAvailabilitySchema,
  RuntimeAvailabilitySchema,
} from "../src/ai-domain.ts"
import { describeOperation, intervalLimit, sampledFrames } from "../src/ai-helpers.ts"

test("parses a pending official device login without credentials", () => {
  // Given the frozen Codex status response.
  const payload = {
    installed: true,
    version: "0.155.1",
    state: "login_pending",
    auth_mode: "none",
    login: {
      url: "https://auth.openai.com/device",
      code: "ABCD-EFGH",
      expires_at: "2026-09-20T12:00:00Z",
    },
    error: null,
  }
  // When the response crosses the browser boundary.
  const parsed = CodexStatusSchema.parse(payload)
  // Then the public device details survive and no credential field is accepted.
  assert.equal(parsed.login?.code, "ABCD-EFGH")
  assert.equal("token" in parsed, false)
})

test("parses a ready run with a published ChangeSet", () => {
  // Given a completed source-scoped run.
  const payload = {
    id: "run",
    project_id: "project",
    base_revision: "4",
    instruction: "Build an initial visual cut",
    scope: { type: "source", source_id: "source", start_frame: "0", end_frame: "300" },
    status: "ready",
    created_at: "2026-09-20T12:00:00Z",
    updated_at: "2026-09-20T12:01:00Z",
    change_set_id: "change-set",
    summary: "Created three scenes",
    warnings: [],
    sample_count: 12,
    error: null,
  }
  // When parsing the API response.
  const parsed = AiRunSchema.parse(payload)
  // Then proposal publication remains explicit and typed.
  assert.equal(parsed.change_set_id, "change-set")
  assert.equal(parsed.sample_count, 12)
  assert.equal(parsed.provider, "codex")
})

test("parses public provider configuration without a credential field", () => {
  const parsed = ProviderStatusesSchema.parse({
    providers: [
      {
        id: "openai-compatible",
        name: "OpenAI-compatible API",
        configured: true,
        model: "vision-model",
        endpoint: "https://api.example.test/v1",
        detail: null,
        api_key: "must-not-cross-the-boundary",
      },
    ],
  })

  assert.equal(parsed.providers[0]?.configured, true)
  assert.equal("api_key" in parsed.providers[0], false)
})

test("keeps an explicit API provider on a request", () => {
  const parsed = AiRequestSchema.parse({
    provider: "openai-compatible",
    base_revision: "4",
    instruction: "Build an initial visual cut",
    consent: true,
    scope: { type: "source", source_id: "source", start_frame: "0", end_frame: "300" },
  })

  assert.equal(parsed.provider, "openai-compatible")
})

test("parses PART availability while stripping undeclared fields", () => {
  // Given a runtime response carrying a field the browser must never retain.
  const payload = {
    available: true,
    engine: "@slop-lab/part",
    version: "0.1.0",
    error: null,
    token: "must-not-cross-the-boundary",
  }
  // When the response crosses the browser boundary.
  const parsed = RuntimeAvailabilitySchema.parse(payload)
  // Then only the public availability contract remains.
  assert.equal(parsed.version, "0.1.0")
  assert.equal("token" in parsed, false)
})

test("parses allowlisted per-run identity and progress without secret fields", () => {
  // Given a live PART observation with an undeclared provider payload.
  const payload = {
    run_id: "run",
    available: true,
    runtime: {
      engine: "@slop-lab/part",
      engine_version: "0.1.0",
      execution_id: "execution-7",
      profile_revision: "profile-3",
      api_key: "must-not-cross-the-boundary",
    },
    progress: { sequence: 4, phase: "validating", prompt: "private" },
  }
  // When the response crosses the browser boundary.
  const parsed = RunRuntimeAvailabilitySchema.parse(payload)
  // Then fixed identity and progress survive while unknown data is stripped.
  assert.equal(parsed.runtime?.execution_id, "execution-7")
  assert.equal(parsed.progress?.sequence, 4)
  assert.equal("api_key" in parsed.runtime, false)
  assert.equal("prompt" in parsed.progress, false)
})

test("rejects an unknown PART progress phase", () => {
  // Given a phase outside the fixed public allowlist.
  const payload = {
    run_id: "run",
    available: true,
    runtime: {
      engine: "@slop-lab/part",
      engine_version: "0.1.0",
      execution_id: "execution-7",
      profile_revision: "profile-3",
    },
    progress: { sequence: 5, phase: "streaming_prompt" },
  }
  // When parsing it, then schema drift is rejected rather than rendered.
  assert.equal(RunRuntimeAvailabilitySchema.safeParse(payload).success, false)
})

test("keeps unavailable legacy runtime observations nonfatal and empty", () => {
  // Given a persisted run whose bounded runtime observation was evicted.
  const payload = { run_id: "legacy-run", available: false, runtime: null, progress: null }
  // When parsing the response.
  const parsed = RunRuntimeAvailabilitySchema.parse(payload)
  // Then the run remains addressable without fabricated metadata.
  assert.equal(parsed.run_id, "legacy-run")
  assert.equal(parsed.available, false)
  assert.equal(parsed.runtime, null)
})

test("bounds source intervals by media duration and six hours", () => {
  // Given media longer and shorter than the service ceiling.
  // When calculating selectable interval limits.
  const longLimit = intervalLimit("9999999", 60)
  const shortLimit = intervalLimit("900", 30)
  // Then the actual source duration wins until the six-hour ceiling applies.
  assert.equal(longLimit, 1_296_000)
  assert.equal(shortLimit, 900)
})

test("discloses the same bucket-center frames sampled by the backend", () => {
  // Given a long source selection.
  // When deriving the disclosed sampling plan.
  const frames = sampledFrames(30, 3_630, 12)
  // Then the disclosed frames match the backend's bounded sampling contract.
  assert.equal(frames.length, 12)
  assert.equal(frames[0], 180)
  assert.equal(frames.at(-1), 3_480)
})

test("describes operation source, timing, and target rather than only its type", () => {
  // Given an inserted scene proposal.
  const operation = {
    id: "operation",
    scene_id: "new-scene",
    type: "insert_scene",
    source_id: "source-a",
    in_frame: "30",
    out_frame: "90",
    title: "Opening",
    rationale: "Start on the clearest shot",
  }
  // When formatting it for review.
  const detail = describeOperation(operation, 30)
  // Then the human-readable result exposes its actual edit target and timing.
  assert.match(detail, /Opening/)
  assert.match(detail, /source-a/)
  assert.match(detail, /00:00:01:00/)
  assert.match(detail, /00:00:03:00/)
})
