import assert from "node:assert/strict";
import { copyFile, chmod, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { randomUUID } from "node:crypto";
import { watch } from "node:fs";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Auth } from "../src/auth.ts";
import { Jobs } from "../src/jobs.ts";
import type { CodexStatus } from "../src/types.ts";

async function fixture(t: TestContext, scenario: { readonly auth: string; readonly mode: string }) {
  const root = await mkdtemp(join(tmpdir(), "longcut-codex-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const home = join(root, "auth");
  const runs = join(root, "runs");
  const executable = join(root, "fake-codex");
  await mkdir(home); await mkdir(runs);
  await writeFile(join(home, "scenario.json"), JSON.stringify(scenario));
  await copyFile(new URL("fixture.mjs", import.meta.url), executable);
  await chmod(executable, 0o700);
  const auth = new Auth(executable, home);
  return { jobs: new Jobs({ executable, home, runs, auth }), auth, runs, home };
}
const request = () => ({ id: randomUUID(), prompt: "A test request", images: [], schema: { type: "object" } });

test("a valid tool-free fixture is parsed and its temporary inputs are removed", async (t) => {
  const { jobs, runs } = await fixture(t, { auth: "chatgpt", mode: "valid" });
  const initial = await jobs.start(request());
  await jobs.waitForIdle();
  assert.equal(jobs.get(initial.id).status, "succeeded");
  assert.equal(jobs.get(initial.id).result?.operations.length, 0);
  assert.deepEqual(await readdir(runs), []);
});

for (const mode of ["invalid", "oversize", "tool"]) {
  test(`a ${mode} fixture never becomes an accepted proposal`, async (t) => {
    const { jobs } = await fixture(t, { auth: "chatgpt", mode });
    const initial = await jobs.start(request());
    await jobs.waitForIdle();
    assert.equal(jobs.get(initial.id).status, "failed");
    assert.equal(jobs.get(initial.id).result, null);
  });
}

test("API-key login cannot start a subscription editing job", async (t) => {
  const { jobs, auth } = await fixture(t, { auth: "api", mode: "valid" });
  assert.equal((await auth.status()).auth_mode, "unsupported");
  await assert.rejects(jobs.start(request()), { code: "AUTH_REQUIRED" });
  assert.equal(jobs.busy(), false);
});

test("late successful output cannot override cancellation", { timeout: 5000 }, async (t) => {
  const { jobs, home } = await fixture(t, { auth: "chatgpt", mode: "late" });
  const watcher = watch(home);
  t.after(() => watcher.close());
  const started = once(watcher, "change");
  const initial = await jobs.start(request());
  await started;
  await jobs.cancel(initial.id);
  await jobs.waitForIdle();
  assert.equal(jobs.get(initial.id).status, "cancelled");
  assert.equal(jobs.get(initial.id).result, null);
});

test("concurrent jobs sharing an account are rejected", async (t) => {
  const { jobs } = await fixture(t, { auth: "chatgpt", mode: "late" });
  const initial = await jobs.start(request());
  await assert.rejects(jobs.start(request()), { code: "CODEX_BUSY" });
  await jobs.cancel(initial.id);
  await jobs.waitForIdle();
});

test("shutdown prevents generation that was still checking account status", async (t) => {
  const { jobs, auth, runs } = await fixture(t, { auth: "chatgpt", mode: "valid" });
  const barrier = Promise.withResolvers<CodexStatus>();
  t.mock.method(auth, "status", () => barrier.promise);
  const starting = jobs.start(request());
  const rejected = assert.rejects(starting, { code: "SHUTTING_DOWN" });
  const stopped = jobs.shutdown();
  barrier.resolve({ installed: true, version: "fixture", state: "ready", auth_mode: "chatgpt", login: null, error: null });
  await rejected;
  await stopped;
  assert.deepEqual(await readdir(runs), []);
  await assert.rejects(jobs.start(request()), { code: "SHUTTING_DOWN" });
});
