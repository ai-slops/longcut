import * as fs from "node:fs/promises";
import { chmod, copyFile, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { TestContext } from "node:test";
import { CodexTransport, type CodexTransportDependencies } from "../src/codex-transport.ts";
import { codexAgent } from "../src/model.ts";
import { run } from "../src/process.ts";

export async function fixture(t: TestContext, mode: string) {
  const root = await mkdtemp(join(tmpdir(), "longcut-part-codex-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const home = join(root, "auth");
  const runs = join(root, "runs");
  const executable = join(root, "fake-codex");
  await mkdir(home); await mkdir(runs);
  await writeFile(join(home, "scenario.json"), JSON.stringify({ auth: "chatgpt", mode }));
  await copyFile(new URL("fixture.mjs", import.meta.url), executable);
  await chmod(executable, 0o700);
  const transport = new CodexTransport({ executable, home, runs });
  return { agent: codexAgent(transport), transport, runs, executable, home };
}

export const request = () => ({ id: crypto.randomUUID(), provider: "codex" as const, prompt: "Fixture", schema: { type: "object" }, images: [] });
export const remainsPendingForTurn = async (promise: Promise<unknown>): Promise<boolean> => Promise.race([
  promise.then(() => false),
  new Promise<boolean>((resolve) => setImmediate(() => resolve(true))),
]);
export const dependencies = (overrides: Partial<CodexTransportDependencies>): CodexTransportDependencies => ({
  mkdtemp: fs.mkdtemp,
  readFile: fs.readFile,
  rm: fs.rm,
  stat: fs.stat,
  writeFile: fs.writeFile,
  launch: run,
  ...overrides,
});
