import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { setTimeout as delay } from "node:timers/promises";

const publicBase = process.env.LONGCUT_URL ?? "http://127.0.0.1:3020";
const controlBase = process.env.PART_FIXTURE_URL ?? "http://agent-dind:4111";
const apiBase = `${publicBase}/api/v1`;

async function request(path, options = {}) {
  const response = await fetch(`${apiBase}${path}`, { ...options, signal: AbortSignal.timeout(120_000) });
  const text = await response.text();
  assert(response.ok, `${path}: ${response.status} ${text}`);
  return text ? JSON.parse(text) : null;
}

async function post(path, body, expected = 200) {
  const response = await fetch(`${apiBase}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(120_000),
  });
  const text = await response.text();
  assert.equal(response.status, expected, `${path}: ${response.status} ${text}`);
  return text ? JSON.parse(text) : null;
}

async function control(path, body) {
  const response = await fetch(`${controlBase}${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: body === undefined ? {} : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(30_000),
  });
  assert.equal(response.ok, true, `${path}: ${response.status} ${await response.clone().text()}`);
  return response.json();
}

async function until(label, operation, predicate, attempts = 240) {
  for (let attempt = 0; attempt < attempts; attempt++) {
    const value = await operation();
    if (predicate(value)) return value;
    await delay(500);
  }
  assert.fail(`Timed out waiting for ${label}`);
}

async function createDemo(title) {
  let project = await post("/projects", { title, frame_rate: 30 });
  const path = `/projects/${project.id}`;
  await post(`${path}/demo`, {});
  project = await until(`${title} media`, () => request(path), (value) => value.sources.length === 1 && value.sources[0].status !== "processing");
  assert.equal(project.sources[0].status, "ready", project.sources[0].error);
  return project;
}

async function command(project, value) {
  return post(`/projects/${project.id}/commands`, {
    command_id: randomUUID(),
    base_revision: project.revision,
    command: value,
  });
}

async function startRun(project, provider) {
  const source = project.sources[0];
  return post(`/projects/${project.id}/ai/runs`, {
    provider,
    base_revision: project.revision,
    instruction: "Create one evidence-backed synthetic acceptance edit.",
    consent: true,
    scope: { type: "source", source_id: source.id, start_frame: "0", end_frame: "90" },
  }, 202);
}

async function runState(projectId, runId, statuses) {
  return until(`run ${runId} ${statuses.join("/")}`, async () => {
    const runs = await request(`/projects/${projectId}/ai/runs`);
    return runs.find((candidate) => candidate.id === runId);
  }, (run) => run && statuses.includes(run.status));
}

function sceneSnapshot(project) {
  return project.scenes.map(({ id, source_id, in_frame, out_frame, title }) => ({ id, source_id, in_frame, out_frame, title }));
}

const startedAt = new Date().toISOString();
const runtimeInfo = await request("/ai/runtime");
assert.deepEqual({ available: runtimeInfo.available, engine: runtimeInfo.engine, version: runtimeInfo.version }, {
  available: true,
  engine: "@slop-lab/part",
  version: "0.1.0",
});
const providers = await request("/ai/providers");
assert(providers.providers.every((provider) => provider.configured && provider.detail.includes("test_provider=true")));

await control("/control/scenario", { mode: "ready", response: "sse" });
let compatibleProject = await createDemo("PART compatible ready acceptance");
const compatibleBefore = sceneSnapshot(compatibleProject);
const compatibleRun = await startRun(compatibleProject, "openai-compatible");
const compatibleReady = await runState(compatibleProject.id, compatibleRun.id, ["ready"]);
const compatibleRuntime = await request(`/projects/${compatibleProject.id}/ai/runs/${compatibleRun.id}/runtime`);
compatibleProject = await request(`/projects/${compatibleProject.id}`);
assert.deepEqual(sceneSnapshot(compatibleProject), compatibleBefore);
const compatibleChange = compatibleProject.change_sets.find((item) => item.id === compatibleReady.change_set_id);
assert(compatibleChange && compatibleChange.status === "ready" && compatibleChange.operations.length > 0);
compatibleProject = await command(compatibleProject, {
  type: "apply_change_set",
  change_set_id: compatibleChange.id,
  operation_ids: compatibleChange.operations.map((operation) => operation.id),
});
assert.notDeepEqual(sceneSnapshot(compatibleProject), compatibleBefore);
const compatibleFixture = await control("/control/state");

await control("/control/scenario", { mode: "ready", response: "json" });
let codexProject = await createDemo("PART Codex ready browser review");
const codexBefore = sceneSnapshot(codexProject);
const codexRun = await startRun(codexProject, "codex");
const codexReady = await runState(codexProject.id, codexRun.id, ["ready"]);
const codexRuntime = await request(`/projects/${codexProject.id}/ai/runs/${codexRun.id}/runtime`);
codexProject = await request(`/projects/${codexProject.id}`);
assert.deepEqual(sceneSnapshot(codexProject), codexBefore);
const codexChange = codexProject.change_sets.find((item) => item.id === codexReady.change_set_id);
assert(codexChange && codexChange.status === "ready" && codexChange.operations.length > 0);
const codexFixture = await control("/control/state");

await control("/control/scenario", { mode: "hold", response: "json" });
let cancelProject = await createDemo("PART cancellation acceptance");
const cancelBefore = sceneSnapshot(cancelProject);
const cancelRun = await startRun(cancelProject, "openai-compatible");
await runState(cancelProject.id, cancelRun.id, ["generating"]);
const cancelAdmittedRuntime = await request(`/projects/${cancelProject.id}/ai/runs/${cancelRun.id}/runtime`);
const cancelled = await post(`/projects/${cancelProject.id}/ai/runs/${cancelRun.id}/cancel`, {});
assert.equal(cancelled.status, "cancelled");
const cancelTerminalRuntime = await until("cancel runtime terminal", () => request(`/projects/${cancelProject.id}/ai/runs/${cancelRun.id}/runtime`), (value) => value.progress?.phase === "terminal");
cancelProject = await request(`/projects/${cancelProject.id}`);
assert.deepEqual(sceneSnapshot(cancelProject), cancelBefore);
assert.equal(cancelProject.change_sets.length, 0);
const cancelFixture = await until("cancel fixture cleanup", () => control("/control/state"), (value) => value.active_http === 0);

await control("/control/scenario", { mode: "hold", response: "sse" });
let staleProject = await createDemo("PART stale acceptance");
const staleBefore = sceneSnapshot(staleProject);
const staleRun = await startRun(staleProject, "openai-compatible");
await runState(staleProject.id, staleRun.id, ["generating"]);
staleProject = await command(staleProject, { type: "set_brief", brief: "Manual revision while provider is held." });
await control("/control/release", {});
const stale = await runState(staleProject.id, staleRun.id, ["stale"]);
staleProject = await request(`/projects/${staleProject.id}`);
assert.deepEqual(sceneSnapshot(staleProject), staleBefore);
assert.equal(staleProject.change_sets.length, 0);
const staleFixture = await until("stale fixture cleanup", () => control("/control/state"), (value) => value.active_http === 0);

const proof = {
  passed: true,
  started_at: startedAt,
  completed_at: new Date().toISOString(),
  node_version: process.version,
  runtime: runtimeInfo,
  providers,
  test_provider: true,
  auth_header_public_metadata: false,
  compatible: {
    project_id: compatibleProject.id,
    run_id: compatibleRun.id,
    change_set_id: compatibleChange.id,
    runtime: compatibleRuntime,
    project_before: compatibleBefore,
    project_after_ready: compatibleBefore,
    project_after_apply: sceneSnapshot(compatibleProject),
    fixture: compatibleFixture,
  },
  codex: {
    project_id: codexProject.id,
    run_id: codexRun.id,
    change_set_id: codexChange.id,
    runtime: codexRuntime,
    project_before: codexBefore,
    project_after_ready: sceneSnapshot(codexProject),
    fixture: codexFixture,
    left_ready_for_browser_qa: true,
  },
  cancellation: {
    project_id: cancelProject.id,
    run_id: cancelRun.id,
    status: cancelled.status,
    runtime_admitted: cancelAdmittedRuntime,
    runtime_terminal: cancelTerminalRuntime,
    no_change_set: cancelProject.change_sets.length === 0,
    scenes_unchanged: JSON.stringify(sceneSnapshot(cancelProject)) === JSON.stringify(cancelBefore),
    fixture: cancelFixture,
  },
  stale: {
    project_id: staleProject.id,
    run_id: staleRun.id,
    status: stale.status,
    revision_after_manual_change: staleProject.revision,
    no_change_set: staleProject.change_sets.length === 0,
    scenes_unchanged: JSON.stringify(sceneSnapshot(staleProject)) === JSON.stringify(staleBefore),
    fixture: staleFixture,
  },
  urls: {
    browser: publicBase,
    core: "http://agent-dind:4020",
    runtime_health: "http://agent-dind:4110/health",
    fixture_control: controlBase,
  },
};
assert(!JSON.stringify(proof).includes("fixture-only-key"));
await mkdir(new URL("./evidence/", import.meta.url), { recursive: true });
await writeFile(new URL("./evidence/proof.json", import.meta.url), `${JSON.stringify(proof, null, 2)}\n`);
await writeFile(new URL("./evidence/proof.md", import.meta.url), `# PART runtime acceptance proof\n\n- Passed: true\n- Engine: ${runtimeInfo.engine} ${runtimeInfo.version}\n- Compatible run: \`${compatibleRun.id}\`, execution \`${compatibleRuntime.runtime.execution_id}\`\n- Codex run: \`${codexRun.id}\`, execution \`${codexRuntime.runtime.execution_id}\`\n- Browser-ready project: \`${codexProject.id}\`, ChangeSet \`${codexChange.id}\`\n- Cancellation: \`${cancelRun.id}\` -> \`${cancelled.status}\`, runtime phase \`${cancelTerminalRuntime.progress.phase}\`\n- Stale: \`${staleRun.id}\` -> \`${stale.status}\`\n- Browser URL: ${publicBase}\n`);
process.stdout.write(`${JSON.stringify(proof, null, 2)}\n`);
