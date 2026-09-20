import assert from "node:assert/strict";
import test from "node:test";
import { run } from "../src/process.ts";

test("child processes receive an allowlist rather than host credentials", async () => {
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

test("an output flood is terminated before buffers grow without a bound", async () => {
  const output = await run(process.execPath, ["-e", "process.stdout.write('x'.repeat(2_000_000));setInterval(()=>{},1000)"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 5000 }).done;
  assert.equal(output.reason, "output_limit");
  assert(output.stdout.length <= 1_048_576);
});

test("a hanging child is terminated at the configured deadline", async () => {
  const output = await run(process.execPath, ["-e", "setInterval(()=>{},1000)"], { cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 100 }).done;
  assert.equal(output.reason, "timeout");
});

test("cancellation closes inherited pipes from the complete process group", { timeout: 5000 }, async () => {
  let ready: (() => void) | undefined;
  const started = new Promise<void>((resolve) => { ready = resolve; });
  const child = run(process.execPath, ["-e", "require('node:child_process').spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'});console.log('ready');setInterval(()=>{},1000)"], {
    cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 3000, onOutput: () => ready?.(),
  });
  await started;
  child.cancel();
  assert.equal((await child.done).reason, "cancelled");
});

test("a rejected event kills the process and cannot become a successful result", async () => {
  const output = await run(process.execPath, ["-e", "console.log('tool');setInterval(()=>{},1000)"], {
    cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 5000,
    onOutput: () => { throw new TypeError("fixture policy rejection"); },
  }).done;
  assert.equal(output.reason, "policy");
});

test("a closed stdin cannot masquerade as a successful zero-exit generation", async () => {
  const output = await run(process.execPath, ["-e", "process.on('SIGTERM',()=>{console.log(JSON.stringify({type:'item.started',item:{type:'file_change'}}));process.exit(0)});require('node:fs').closeSync(0);setInterval(()=>{},1000)"], {
    cwd: "/tmp", home: "/tmp/fixture", timeoutMs: 5000, input: "편집".repeat(64000),
  }).done;
  assert.equal(output.reason, "io_error");
});
