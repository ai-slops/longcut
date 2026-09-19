/** Live API acceptance driver. Creates an isolated synthetic project; never deletes user data. */
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { ProjectSchema } from "../apps/web/src/domain.ts";

const base = process.env.LONGCUT_URL ?? "http://127.0.0.1:3001";
const endpoint = `${base}/api/v1`;
async function api(path, body) {
  const result = await fetch(`${endpoint}${path}`, {
    ...(body === undefined ? {} : { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(120_000),
  });
  assert.equal(result.ok, true, `${path}: ${result.status} ${result.ok ? "" : await result.text()}`);
  const payload = await result.json();
  if (payload !== null && typeof payload === "object" && "sources" in payload) {
    return ProjectSchema.parse(payload);
  }
  return payload;
}

async function until(path, predicate) {
  const deadline = Date.now() + 180_000;
  while (Date.now() < deadline) {
    const result = await api(path);
    if (predicate(result)) return result;
    await delay(1_000);
  }
  assert.fail(`Timed out waiting for ${path}`);
}

// Given a real healthy core and an empty, isolated project.
const health = await api("/health");
assert.equal(health.status, "ok");
let project = await api("/projects", { title: `Acceptance ${new Date().toISOString()}`, frame_rate: 30 });
const path = `/projects/${project.id}`;
async function command(command) {
  project = await api(`${path}/commands`, { command_id: randomUUID(), base_revision: project.revision, command });
  return project;
}
async function rejected(command, status, revision = project.revision) {
  const result = await fetch(`${endpoint}${path}/commands`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ command_id: randomUUID(), base_revision: revision, command }),
  });
  assert.equal(result.status, status, await result.text());
}

// When synthetic original media is generated and a second source is streamed through multipart.
project = await api(`${path}/demo`, {});
project = await until(path, (p) => p.sources.length > 0 && p.sources.every((s) => s.status !== "processing"));
const primary = project.sources[0];
assert.equal(primary.status, "ready", primary.error);
assert(BigInt(primary.duration_frames) >= 90n);
assert(primary.proxy_url && primary.thumbnail_url && primary.waveform.length > 0);
const original = await fetch(`${base}${primary.media_url}`);
assert.equal(original.ok, true);
const upload = new FormData();
upload.append("file", await original.blob(), "acceptance-camera.mp4");
const uploaded = await fetch(`${endpoint}${path}/imports?role=camera`, { method: "POST", body: upload, signal: AbortSignal.timeout(120_000) });
assert.equal(uploaded.ok, true, uploaded.ok ? "" : await uploaded.text());
project = await uploaded.json();
project = await until(path, (p) => p.sources.length >= 2 && p.sources.every((s) => s.status !== "processing"));
const cameraSource = project.sources.find((s) => s.id !== primary.id);
assert(cameraSource && cameraSource.status === "ready");

// Then real media seeking survives the same-origin gateway.
const range = await fetch(`${base}${primary.proxy_url}`, { headers: { Range: "bytes=0-31" } });
assert.equal(range.status, 206);
assert.equal((await range.arrayBuffer()).byteLength, 32);

// When three non-overlapping source spans are edited through the command API.
for (let index = 0; index < 3; index++) {
  await command({ type: "insert_scene", source_id: primary.id, in_frame: String(index * 30), out_frame: String((index + 1) * 30), title: `Scene ${index + 1}` });
}
const firstId = project.scenes[0].id;
const secondId = project.scenes[1].id;
await command({ type: "trim_scene", scene_id: firstId, in_frame: "0", out_frame: "25" });
await command({ type: "split_scene", scene_id: secondId, offset_frame: "15" });
assert.equal(project.scenes.length, 4);
const splitRevision = BigInt(project.revision);
await command({ type: "undo" });
assert.equal(project.scenes.length, 3);
assert(BigInt(project.revision) > splitRevision);
await command({ type: "redo" });
assert.equal(project.scenes.length, 4);
await command({ type: "undo" });
await command({ type: "move_scene", scene_id: firstId, before_scene_id: null });
assert.equal(project.scenes.at(-1).id, firstId);
await command({ type: "undo" });
assert.equal(project.scenes[0].id, firstId);

// Then locks, stale revisions and invalid ranges cannot silently change the project.
await command({ type: "lock_scene", scene_id: firstId, locked: true });
await rejected({ type: "trim_scene", scene_id: firstId, in_frame: "0", out_frame: "24" }, 423);
await command({ type: "lock_scene", scene_id: firstId, locked: false });
await rejected({ type: "set_brief", brief: "stale" }, 409, "0");
await rejected({ type: "trim_scene", scene_id: firstId, in_frame: "30", out_frame: "20" }, 422);

// When a range/pin comment and a two-operation proposal are created.
await command({ type: "create_feedback", scene_id: firstId, start_frame: "0", end_frame: "10", body: "Keep the source UI clear.", pin: { x: 0.8, y: 0.8 }, render_id: null });
await command({ type: "update_scene", scene_id: firstId, title: "Opening", purpose: "Establish context", caption: "Synthetic acceptance media" });
const camera = { source_id: cameraSource.id, in_frame: "0", x: 0.72, y: 0.06, width: 0.24, height: 0.24, opacity: 1 };
const cameraOp = randomUUID();
const trimOp = randomUUID();
await command({ type: "create_change_set", title: "Review camera placement", intent: "Move the camera without changing the manual cut", evidence: [firstId], operations: [
  { id: cameraOp, scene_id: firstId, type: "set_camera", camera, rationale: "Avoid lower UI" },
  { id: trimOp, scene_id: secondId, type: "trim_scene", in_frame: "31", out_frame: "59", rationale: "Optional tighter timing" },
] });
const proposalId = project.change_sets.at(-1).id;
await command({ type: "apply_change_set", change_set_id: proposalId, operation_ids: [cameraOp] });
assert.equal(project.scenes[0].out_frame, "25");
assert.equal(project.scenes[0].camera.x, camera.x);
assert.equal(project.scenes[1].in_frame, "30");
assert.equal(project.change_sets.at(-1).status, "partially-applied");
assert.equal(project.feedback[0].scene_id, firstId);
await command({ type: "reject_change_set", change_set_id: proposalId });

// Then retries are idempotent and do not advance the revision twice.
const retry = { command_id: randomUUID(), base_revision: project.revision, command: { type: "set_brief", brief: "Preserve manually reviewed timing." } };
const once = await api(`${path}/commands`, retry);
const twice = await api(`${path}/commands`, retry);
assert.equal(once.revision, twice.revision);
project = twice;

// When final rendering starts, later edits must not change its frozen revision.
const frozenRevision = project.revision;
const render = await api(`${path}/renders`, { base_revision: frozenRevision, kind: "final" });
await command({ type: "set_brief", brief: "A later edit must not alter the queued render." });
project = await until(path, (p) => p.renders.some((r) => r.id === render.id && ["ready", "error"].includes(r.status)));
const output = project.renders.find((r) => r.id === render.id);
assert.equal(output.status, "ready", output.error);
assert.equal(output.revision, frozenRevision);
for (const name of ["media_url", "srt_url", "vtt_url", "lineage_url"]) {
  assert(output[name]);
  const file = await fetch(`${base}${output[name]}`);
  assert.equal(file.ok, true, name);
  assert((await file.arrayBuffer()).byteLength > 0, name);
}
const reopened = await api(path);
assert.equal(reopened.feedback[0].body, "Keep the source UI clear.");
assert.equal(reopened.scenes[0].camera.source_id, cameraSource.id);
process.stdout.write(JSON.stringify({ passed: true, project_id: project.id, revision: project.revision, render: output }, null, 2) + "\n");
