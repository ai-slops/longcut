#!/usr/bin/env node
import assert from "node:assert/strict";
import { appendFile, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { inspectJpegBase64, proposalFor, sourceFrames } from "./fixture-contract.mjs";

const home = process.env.CODEX_HOME;
assert(home, "CODEX_HOME is required");
const scenarioPath = join(home, "scenario.json");
const capturesPath = join(home, "codex-captures.jsonl");
const args = process.argv.slice(2);

if (args.includes("login") && args.includes("status")) {
  process.stderr.write("Logged in using ChatGPT\n");
  process.exit(0);
}

assert(args.includes("exec"), "only login status and exec are supported");
const option = (name) => {
  const index = args.indexOf(name);
  assert(index >= 0 && args[index + 1], `${name} is required`);
  return args[index + 1];
};
const schemaPath = option("--output-schema");
const outputPath = option("--output-last-message");
const imagePaths = args.flatMap((value, index) => value === "--image" && args[index + 1] ? [args[index + 1]] : []);
const schema = JSON.parse(await readFile(schemaPath, "utf8"));
assert.equal(schema.type, "object");
for (const imagePath of imagePaths) inspectJpegBase64((await readFile(imagePath)).toString("base64"));

let prompt = "";
process.stdin.setEncoding("utf8");
for await (const chunk of process.stdin) prompt += chunk;
await appendFile(capturesPath, `${JSON.stringify({
  provider: "codex",
  test_provider: true,
  argv_policy: {
    ephemeral: args.includes("--ephemeral"),
    strict_config: args.includes("--strict-config"),
    output_schema: true,
    image_count: imagePaths.length,
  },
  source_frames: sourceFrames(prompt),
})}\n`);

let cancelled = false;
process.on("SIGTERM", () => {
  cancelled = true;
  void appendFile(capturesPath, `${JSON.stringify({ provider: "codex", test_provider: true, cancelled: true })}\n`).finally(() => process.exit(143));
});
while (!cancelled) {
  const scenario = JSON.parse(await readFile(scenarioPath, "utf8"));
  if (scenario.mode !== "hold" || scenario.release === true) break;
  await delay(50);
}
if (!cancelled) {
  await writeFile(outputPath, JSON.stringify(proposalFor(prompt)));
  process.stdout.write('{"type":"item.completed","item":{"type":"agent_message"}}\n');
}
