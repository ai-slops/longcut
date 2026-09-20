import assert from "node:assert/strict";
import { once } from "node:events";
import { request } from "node:http";
import test, { type TestContext } from "node:test";
import { createAdapter } from "../src/http.ts";
import { ServiceError, type CodexStatus, type Job, type JobRequest } from "../src/types.ts";

const token = "c".repeat(64);
const headers = { authorization: `Bearer ${token}`, "content-type": "application/json" };
const status: CodexStatus = { installed: true, version: "fixture", state: "ready", auth_mode: "chatgpt", login: null, error: null };
async function fixture(t: TestContext) {
  const records = new Map<string, Job>();
  let logoutCalls = 0;
  const auth = {
    cached: () => status, status: async () => status, pending: () => false,
    login: async () => status, cancel: async () => status,
    logout: async () => { logoutCalls++; return status; }, shutdown: async () => {},
  };
  const jobs = {
    busy: () => false,
    get: (id: string): Job => {
      const job = records.get(id);
      if (!job) throw new ServiceError("NOT_FOUND", 404, "Fixture not found");
      return job;
    },
    start: async (input: JobRequest): Promise<Job> => {
      const job: Job = { id: input.id, status: "running", result: null, error: null };
      records.set(input.id, job); return job;
    },
    cancel: async (id: string): Promise<Job> => ({ id, status: "cancelled", result: null, error: null }),
    shutdown: async () => {},
  };
  const adapter = createAdapter({ auth, jobs, token: Buffer.from(token), safety: "explicit test fixture" });
  adapter.server.listen(0, "127.0.0.1");
  await once(adapter.server, "listening");
  const address = adapter.server.address();
  assert(address && typeof address === "object");
  t.after(() => adapter.shutdown());
  return { ...adapter, url: `http://127.0.0.1:${address.port}`, logoutCalls: () => logoutCalls, records };
}

test("internal endpoints require an exact Bearer authorization scheme", async (t) => {
  const { url } = await fixture(t);
  assert.equal((await fetch(`${url}/status`)).status, 401);
  assert.equal((await fetch(`${url}/status`, { headers: { authorization: token } })).status, 401);
  assert.equal((await fetch(`${url}/status`, { headers })).status, 200);
});

test("a paused job body reserves the account before logout can start", { timeout: 5000 }, async (t) => {
  const app = await fixture(t);
  const payload = JSON.stringify({ id: "11111111-1111-4111-8111-111111111111", prompt: "Fixture", schema: {}, images: [] });
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

test("an oversized context is rejected before job generation", async (t) => {
  const app = await fixture(t);
  const result = await fetch(`${app.url}/jobs`, { method: "POST", headers, body: "x".repeat(16 * 1024 * 1024 + 1) });
  assert.equal(result.status, 413);
  assert.equal(app.records.size, 0);
});

test("uppercase UUID submissions can be retrieved through the polling route", async (t) => {
  const { url } = await fixture(t);
  const id = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
  assert.equal((await fetch(`${url}/jobs`, { method: "POST", headers, body: JSON.stringify({ id, prompt: "Fixture", schema: {}, images: [] }) })).status, 202);
  assert.equal((await fetch(`${url}/jobs/${id}`, { headers })).status, 200);
});
