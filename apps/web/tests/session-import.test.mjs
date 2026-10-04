import assert from "node:assert/strict"
import test from "node:test"
import { ProjectSchema } from "../src/domain.ts"
import { SessionImportJobSchema, SourceSessionSummarySchema } from "../src/session-domain.ts"
import { preparePackageFiles } from "../src/session-files.ts"
import { sourceSessionAssetUrl } from "../src/session-links.ts"

const session = {
  id: "source-session",
  session_id: "capture-session",
  title: "Completed recording",
  duration_ns: "120000000000",
  conformance: "degraded",
  imported_at: "2026-10-03T10:00:00Z",
  assets: [{ asset_id: "asset-video", uri: "media/program.mp4", availability: "online" }],
  sources: [
    {
      source_id: "contract-program",
      display_name: "Program",
      semantic_role: "program",
      kind: "av",
      editor_source_id: "editor-source",
      reason: null,
    },
    {
      source_id: "contract-iso",
      display_name: "Unsupported ISO",
      semantic_role: "camera_iso",
      kind: "video",
      editor_source_id: null,
      reason: "Timing adapter is not supported",
    },
  ],
  warnings: [{ code: "OFFLINE_ASSET", path: "media/iso.mp4", message: "Asset is offline" }],
  verification_url: "/api/v1/projects/project/source-sessions/source-session",
}

test("parses the exact prepared session import contract", () => {
  // Given a prepared degraded job from the API.
  const payload = {
    id: "import-job",
    project_id: "project",
    status: "prepared",
    created_at: "2026-10-03T10:00:00Z",
    error: null,
    session,
  }
  // When it crosses the browser boundary.
  const parsed = SessionImportJobSchema.parse(payload)
  // Then unsupported source reasons and warnings remain reviewable.
  assert.equal(parsed.session?.sources[1]?.reason, "Timing adapter is not supported")
  assert.equal(parsed.session?.warnings[0]?.code, "OFFLINE_ASSET")
})

test("rejects a job status outside the fixed wire contract", () => {
  // Given an invented server status.
  const payload = {
    id: "import-job",
    project_id: "project",
    status: "complete",
    created_at: "2026-10-03T10:00:00Z",
    error: null,
    session,
  }
  // When parsing it.
  const parsed = SessionImportJobSchema.safeParse(payload)
  // Then schema drift is exposed instead of rendered as success.
  assert.equal(parsed.success, false)
})

test("keeps persisted source sessions and defaults older projects to none", () => {
  // Given current and legacy project payloads.
  const base = {
    id: "project",
    title: "Edit",
    revision: "0",
    frame_rate: 30,
    updated_at: "2026-10-03T10:00:00Z",
    brief: "",
    sources: [],
    scenes: [],
    feedback: [],
    change_sets: [],
    renders: [],
    can_undo: false,
    can_redo: false,
  }
  // When both payloads cross the project boundary.
  const current = ProjectSchema.parse({ ...base, source_sessions: [session] })
  const legacy = ProjectSchema.parse(base)
  // Then provenance is retained while old persisted JSON remains readable.
  assert.equal(
    SourceSessionSummarySchema.parse(current.source_sessions[0]).session_id,
    "capture-session",
  )
  assert.deepEqual(legacy.source_sessions, [])
})

test("strips the selected root directory from multipart filenames", () => {
  // Given files selected through a directory picker.
  const manifest = new File(["{}"], "manifest.json")
  const media = new File(["video"], "program.mp4")
  Object.defineProperty(manifest, "webkitRelativePath", { value: "completed/manifest.json" })
  Object.defineProperty(media, "webkitRelativePath", { value: "completed/media/program.mp4" })
  // When preparing package-relative multipart entries.
  const prepared = preparePackageFiles([manifest, media])
  // Then every name has exactly the selected root prefix removed.
  assert.deepEqual(
    prepared.map(({ path }) => path),
    ["manifest.json", "media/program.mp4"],
  )
})

test("rejects a selected directory without the fixed manifest", () => {
  // Given a directory containing media but no manifest.json.
  const media = new File(["video"], "program.mp4")
  Object.defineProperty(media, "webkitRelativePath", { value: "incomplete/media/program.mp4" })
  // When preparing the directory for upload.
  // Then it is rejected locally before any request can claim preparation.
  assert.throws(() => preparePackageFiles([media]), /manifest\.json/)
})

test("encodes immutable active-session asset download identifiers", () => {
  // Given identifiers that cannot be interpolated as raw path segments.
  // When creating the browser download URL.
  const url = sourceSessionAssetUrl("project/one", "session two", "asset#three", true)
  // Then every identifier is encoded independently on the exact API route.
  assert.equal(
    url,
    "/api/v1/projects/project%2Fone/source-sessions/session%20two/assets/asset%23three",
  )
})

test("does not create original asset links for staged or undone sessions", () => {
  // Given a session summary that is not present in active project provenance.
  // When its original asset destination is requested.
  const url = sourceSessionAssetUrl("project", "staged-session", "asset", false)
  // Then the UI has no navigable endpoint for an inactive package.
  assert.equal(url, null)
})
