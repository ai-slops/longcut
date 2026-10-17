import assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import { readdir } from "node:fs/promises";
import test from "node:test";
import { CodexTransport } from "../src/codex-transport.ts";
import { codexAgent, partInput } from "../src/model.ts";
import type { Process } from "../src/process.ts";
import { dependencies, fixture, remainsPendingForTurn, request } from "./codex-fixture.ts";

test("real PART executes the fake Codex CLI and removes temporary inputs", async (t) => {
  const app = await fixture(t, "valid");
  const started = await app.agent.start(await partInput(request()));
  assert.equal(started.ok, true);
  if (!started.ok) return;
  assert.match(started.value.ref.id, /^[0-9a-f-]{36}$/);
  const outcome = await started.value.done;
  assert.equal(outcome.phase, "completed");
  assert.deepEqual(await readdir(app.runs), []);
  await app.agent.dispose();
});

test("Codex tool events and invalid proposals fail through real PART", async (t) => {
  for (const mode of ["tool", "invalid"]) {
    const app = await fixture(t, mode);
    const outcome = await app.agent.generate(await partInput(request()));
    assert.equal(outcome.ok && outcome.value.phase, "failed");
    assert.deepEqual(await readdir(app.runs), []);
    await app.agent.dispose();
  }
});

test("PART cancellation waits for Codex process cleanup and rejects late success", async (t) => {
  const app = await fixture(t, "late");
  const started = await app.agent.start(await partInput(request()));
  assert.equal(started.ok, true);
  if (!started.ok) return;
  await started.value.requestCancel("fixture-cancel");
  await app.transport.shutdown();
  const outcome = await started.value.done;
  assert.notEqual(outcome.phase, "completed");
  assert.deepEqual(await readdir(app.runs), []);
  await app.agent.dispose();
});

test("cancellation during blocked preparation owns cleanup and never launches Codex", async (t) => {
  const app = await fixture(t, "valid");
  await app.agent.dispose();
  const entered = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  const removed = Promise.withResolvers<void>();
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
      rm: async (path, options) => { await fs.rm(path, options); removed.resolve(); },
    }),
  });
  const agent = codexAgent(transport);
  const input = request();
  const started = await agent.start(await partInput(input));
  assert.equal(started.ok, true);
  if (!started.ok) return;
  await entered.promise;
  await started.value.requestCancel("preparation-race");
  assert.equal((await started.value.done).phase, "abandoned");
  const cleanup = transport.cancel(input.id);
  try {
    assert.equal(await remainsPendingForTurn(cleanup), true);
  } finally {
    release.resolve();
    await removed.promise;
  }
  await cleanup;
  assert.equal(spawnCount, 0);
  assert.deepEqual(await readdir(app.runs), []);
  await agent.dispose();
});

test("cleanup failure settles ownership and fails closed without hanging shutdown", async (t) => {
  const app = await fixture(t, "valid");
  await app.agent.dispose();
  const cleanupEntered = Promise.withResolvers<void>();
  const cleanupRelease = Promise.withResolvers<void>();
  const transport = new CodexTransport({
    executable: app.executable,
    home: app.home,
    runs: app.runs,
    dependencies: dependencies({
      rm: async (path, options) => {
        cleanupEntered.resolve();
        await cleanupRelease.promise;
        await fs.rm(path, options);
        throw new Error("fixture cleanup failure");
      },
    }),
  });
  const agent = codexAgent(transport);
  const started = await agent.start(await partInput(request()));
  assert.equal(started.ok, true);
  if (!started.ok) return;
  await cleanupEntered.promise;
  const shutdown = transport.shutdown();
  assert.equal(await remainsPendingForTurn(shutdown), true);
  cleanupRelease.resolve();
  await shutdown;
  const outcome = await started.value.done;
  assert.equal(outcome.phase, "failed");
  assert.deepEqual(await readdir(app.runs), []);
  await agent.dispose();
});
