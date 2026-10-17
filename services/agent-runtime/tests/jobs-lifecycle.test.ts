import assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import { readdir } from "node:fs/promises";
import test from "node:test";
import { Auth } from "../src/auth.ts";
import { CodexTransport } from "../src/codex-transport.ts";
import { Jobs } from "../src/jobs.ts";
import { codexAgent } from "../src/model.ts";
import type { Process } from "../src/process.ts";
import type { CodexStatus } from "../src/types.ts";
import { dependencies, fixture, remainsPendingForTurn, request } from "./codex-fixture.ts";

test("job cancellation returns only after PART and child cleanup are terminal", async (t) => {
  const app = await fixture(t, "late");
  const jobs = new Jobs({ auth: new Auth(app.executable, app.home), codex: app.agent, codexTransport: app.transport, openAi: null, openAiTransport: null });
  const input = request();
  const admitted = await jobs.start(input);
  assert.equal(admitted.runtime.execution_id.length, 36);
  await assert.rejects(jobs.start(request()), { code: "RUNTIME_BUSY" });
  const cancelled = await jobs.cancel(input.id);
  assert.equal(cancelled.status, "cancelled");
  assert.equal(cancelled.progress.phase, "terminal");
  assert.equal(jobs.busy(), false);
  assert.deepEqual(await readdir(app.runs), []);
  await jobs.shutdown();
});

test("Jobs publishes success only after its cleanup barrier settles", async (t) => {
  const app = await fixture(t, "valid");
  const cleanupEntered = Promise.withResolvers<void>();
  const cleanupRelease = Promise.withResolvers<void>();
  const cancel = app.transport.cancel.bind(app.transport);
  t.mock.method(app.transport, "cancel", async (id: string) => {
    await cancel(id);
    cleanupEntered.resolve();
    await cleanupRelease.promise;
  });
  const jobs = new Jobs({ auth: new Auth(app.executable, app.home), codex: app.agent, codexTransport: app.transport, openAi: null, openAiTransport: null });
  const input = request();
  await jobs.start(input);
  await cleanupEntered.promise;
  try {
    const cleaning = jobs.get(input.id);
    assert.equal(cleaning.status, "running");
    assert.equal(cleaning.result, null);
    assert.equal(cleaning.error, null);
    assert.equal(cleaning.progress.phase, "cleaning_up");
  } finally {
    cleanupRelease.resolve();
  }
  await jobs.waitForIdle();
  const succeeded = jobs.get(input.id);
  assert.equal(succeeded.status, "succeeded");
  assert.equal(succeeded.progress.phase, "terminal");
  await jobs.shutdown();
});

test("shutdown rejects admission still waiting on account status", async (t) => {
  const app = await fixture(t, "valid");
  const auth = new Auth(app.executable, app.home);
  const jobs = new Jobs({ auth, codex: app.agent, codexTransport: app.transport, openAi: null, openAiTransport: null });
  const barrier = Promise.withResolvers<CodexStatus>();
  t.mock.method(auth, "status", () => barrier.promise);
  const starting = jobs.start(request());
  const rejected = assert.rejects(starting, { code: "SHUTTING_DOWN" });
  const stopped = jobs.shutdown();
  barrier.resolve({ installed: true, version: "fixture", state: "ready", auth_mode: "chatgpt", login: null, error: null });
  await rejected;
  await stopped;
  assert.equal(jobs.busy(), false);
  assert.deepEqual(await readdir(app.runs), []);
});

test("concurrent Jobs cancellation callers wait for blocked preparation cleanup", async (t) => {
  const app = await fixture(t, "valid");
  await app.agent.dispose();
  const entered = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  let spawnCount = 0;
  const transport = new CodexTransport({
    executable: app.executable,
    home: app.home,
    runs: app.runs,
    dependencies: dependencies({
      mkdtemp: async (prefix) => { entered.resolve(); await release.promise; return fs.mkdtemp(prefix); },
      launch: (): Process => {
        spawnCount++;
        return { cancel: () => {}, done: Promise.resolve({ code: 0, stdout: "", stderr: "", reason: "exit" }) };
      },
    }),
  });
  const agent = codexAgent(transport);
  const jobs = new Jobs({ auth: new Auth(app.executable, app.home), codex: agent, codexTransport: transport, openAi: null, openAiTransport: null });
  const input = request();
  await jobs.start(input);
  await entered.promise;
  const firstCancellation = jobs.cancel(input.id);
  const secondCancellation = jobs.cancel(input.id);
  try {
    assert.equal(await remainsPendingForTurn(firstCancellation), true);
    assert.equal(await remainsPendingForTurn(secondCancellation), true);
    assert.equal(jobs.get(input.id).status, "cancelled");
    assert.equal(jobs.get(input.id).progress.phase, "cleaning_up");
    assert.equal(jobs.busy(), true);
    await assert.rejects(jobs.start(request()), { code: "RUNTIME_BUSY" });
  } finally {
    release.resolve();
  }
  const [firstCancelled, secondCancelled] = await Promise.all([firstCancellation, secondCancellation]);
  assert.equal(firstCancelled.status, "cancelled");
  assert.equal(firstCancelled.progress.phase, "terminal");
  assert.equal(secondCancelled.status, "cancelled");
  assert.equal(secondCancelled.progress.phase, "terminal");
  assert.equal(jobs.busy(), false);
  assert.equal(spawnCount, 0);
  assert.deepEqual(await readdir(app.runs), []);
  await jobs.shutdown();
});

test("Jobs timeout remains busy until blocked transport cleanup settles", async (t) => {
  const app = await fixture(t, "valid");
  await app.agent.dispose();
  const entered = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  let spawnCount = 0;
  const transport = new CodexTransport({
    executable: app.executable,
    home: app.home,
    runs: app.runs,
    dependencies: dependencies({
      mkdtemp: async (prefix) => { entered.resolve(); await release.promise; return fs.mkdtemp(prefix); },
      launch: (): Process => {
        spawnCount++;
        return { cancel: () => {}, done: Promise.resolve({ code: 0, stdout: "", stderr: "", reason: "exit" }) };
      },
    }),
  });
  const agent = codexAgent(transport);
  const timeoutScheduled = Promise.withResolvers<() => void>();
  const jobs = new Jobs({
    auth: new Auth(app.executable, app.home),
    codex: agent,
    codexTransport: transport,
    openAi: null,
    openAiTransport: null,
    scheduleTimeout: (callback) => {
      timeoutScheduled.resolve(callback);
      return setTimeout(() => {}, 600_000);
    },
  });
  const input = request();
  await jobs.start(input);
  await entered.promise;
  const expire = await timeoutScheduled.promise;
  expire();
  try {
    assert.equal(jobs.busy(), true);
    await assert.rejects(jobs.start(request()), { code: "RUNTIME_BUSY" });
  } finally {
    release.resolve();
  }
  await jobs.waitForIdle();
  const timedOut = jobs.get(input.id);
  assert.equal(timedOut.status, "failed");
  assert.equal(timedOut.error?.code, "TIMEOUT");
  assert.equal(timedOut.progress.phase, "terminal");
  assert.equal(spawnCount, 0);
  assert.deepEqual(await readdir(app.runs), []);
  await jobs.shutdown();
});
