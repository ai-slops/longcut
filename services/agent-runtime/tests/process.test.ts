import assert from "node:assert/strict";
import test from "node:test";
import { run } from "../src/process.ts";

test("Codex children receive an allowlist rather than host credentials", async () => {
  const before = process.env["OPENAI_API_KEY"];
  process.env["OPENAI_API_KEY"] = "test-only-canary";
  try {
    const output = await run(process.execPath, ["-e", "console.log(JSON.stringify(Object.keys(process.env)))"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 5000 }).done;
    const keys: unknown = JSON.parse(output.stdout);
    assert(Array.isArray(keys));
    assert(!keys.includes("OPENAI_API_KEY"));
    assert(!keys.includes("DIM_GIT_TOKEN"));
    assert(keys.includes("CODEX_HOME"));
  } finally {
    if (before === undefined) delete process.env["OPENAI_API_KEY"]; else process.env["OPENAI_API_KEY"] = before;
  }
});

test("output flood and deadline terminate the complete process group", async () => {
  const flood = await run(process.execPath, ["-e", "process.stdout.write('x'.repeat(2_000_000));setInterval(()=>{},1000)"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 5000 }).done;
  assert.equal(flood.reason, "output_limit");
  assert(flood.stdout.length <= 1_048_576);
  const timeout = await run(process.execPath, ["-e", "setInterval(()=>{},1000)"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 50 }).done;
  assert.equal(timeout.reason, "timeout");
});

test("cancellation closes inherited pipes from descendants", { timeout: 5000 }, async () => {
  let ready: (() => void) | undefined;
  const started = new Promise<void>((resolve) => { ready = resolve; });
  const child = run(process.execPath, ["-e", "require('node:child_process').spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'});console.log('ready');setInterval(()=>{},1000)"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 3000, onOutput: () => ready?.() });
  await started;
  child.cancel();
  assert.equal((await child.done).reason, "cancelled");
});

test("policy rejection and stdin failure cannot masquerade as success", async () => {
  const policy = await run(process.execPath, ["-e", "console.log('tool');setInterval(()=>{},1000)"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 5000, onOutput: () => { throw new TypeError("fixture rejection"); } }).done;
  assert.equal(policy.reason, "policy");
  const input = await run(process.execPath, ["-e", "require('node:fs').closeSync(0);setInterval(()=>{},1000)"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 5000, input: "edit".repeat(64_000) }).done;
  assert.equal(input.reason, "io_error");
});
