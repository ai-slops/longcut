import assert from "node:assert/strict";
import { once } from "node:events";
import { request } from "node:http";
import test, { type TestContext } from "node:test";
import { createAdapter } from "../src/http.ts";
import { ServiceError, type CodexStatus, type JobRequest, type RuntimeJob } from "../src/types.ts";

const token = "c".repeat(64);
const headers = { authorization: `Bearer ${token}`, "content-type": "application/json" };
const status: CodexStatus = { installed: true, version: "fixture", state: "ready", auth_mode: "chatgpt", login: null, error: null };
const runtimeJob = (id: string, state: RuntimeJob["status"] = "running"): RuntimeJob => ({
  id, status: state, result: null, error: null,
  runtime: { engine: "@slop-lab/part", engine_version: "0.1.0", execution_id: crypto.randomUUID(), profile_revision: "fixture-v1" },
  progress: { sequence: 0, phase: state === "running" ? "admitted" : "terminal" },
});

async function fixture(t: TestContext) {
  const records = new Map<string, RuntimeJob>();
  let logoutCalls = 0;
  const auth = {
    cached: () => status, status: async () => status, pending: () => false,
    login: async () => status, cancel: async () => status,
    logout: async () => { logoutCalls++; return status; }, shutdown: async () => {},
  };
  const jobs = {
    busy: () => false,
    get: (id: string): RuntimeJob => {
      const job = records.get(id);
      if (!job) throw new ServiceError("NOT_FOUND", 404, "Fixture not found");
      return job;
    },
    start: async (input: JobRequest): Promise<RuntimeJob> => {
      const job = runtimeJob(input.id);
      records.set(input.id, job);
      return job;
    },
    cancel: async (id: string): Promise<RuntimeJob> => runtimeJob(id, "cancelled"),
    shutdown: async () => {},
  };
  const providers = () => [{ id: "codex" as const, name: "Codex", configured: true, model: "fixture", endpoint: null, detail: null }];
  const adapter = createAdapter({ auth, jobs, token: Buffer.from(token), providers });
  adapter.server.listen(0, "127.0.0.1");
  await once(adapter.server, "listening");
  const address = adapter.server.address();
  assert(address && typeof address === "object");
  t.after(() => adapter.shutdown());
  return { ...adapter, url: `http://127.0.0.1:${address.port}`, logoutCalls: () => logoutCalls, records };
}

test("health is public and every introspection route requires exact Bearer auth", async (t) => {
  const { url } = await fixture(t);
  assert.deepEqual(await (await fetch(`${url}/health`)).json(), { status: "ok", engine: "@slop-lab/part", version: "0.1.0" });
  for (const path of ["/runtime", "/providers", "/status"]) assert.equal((await fetch(`${url}${path}`)).status, 401);
  const runtime = await (await fetch(`${url}/runtime`, { headers })).json();
  assert.equal(runtime.protocol_version, 1);
  assert.equal(runtime.version, "0.1.0");
});

test("a paused job body reserves the account before logout", { timeout: 5000 }, async (t) => {
  const app = await fixture(t);
  const payload = JSON.stringify({ id: crypto.randomUUID(), prompt: "Fixture", schema: {}, images: [] });
  const received = once(app.server, "request");
  let client: ReturnType<typeof request> | undefined;
  const result = new Promise<number | undefined>((resolve, reject) => {
    client = request(`${app.url}/jobs`, { method: "POST", headers }, (response) => { response.resume(); resolve(response.statusCode); });
    client.on("error", reject);
    client.write(payload.slice(0, 12));
  });
  await received;
  assert.equal((await fetch(`${app.url}/logout`, { method: "POST", headers })).status, 409);
  assert.equal(app.logoutCalls(), 0);
  client?.end(payload.slice(12));
  assert.equal(await result, 202);
});

test("oversized and malformed bodies never reach generation", async (t) => {
  const app = await fixture(t);
  assert.equal((await fetch(`${app.url}/jobs`, { method: "POST", headers, body: "x".repeat(16 * 1024 * 1024 + 1) })).status, 413);
  assert.equal((await fetch(`${app.url}/jobs`, { method: "POST", headers, body: "{" })).status, 422);
  assert.equal(app.records.size, 0);
});

test("provider defaults and uppercase UUID polling remain compatible", async (t) => {
  const { url } = await fixture(t);
  const id = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
  const created = await fetch(`${url}/jobs`, { method: "POST", headers, body: JSON.stringify({ id, prompt: "Fixture", schema: {}, images: [] }) });
  assert.equal(created.status, 202);
  const body = await created.json();
  assert.equal(body.runtime.engine, "@slop-lab/part");
  assert.equal((await fetch(`${url}/jobs/${id}`, { headers })).status, 200);
});
