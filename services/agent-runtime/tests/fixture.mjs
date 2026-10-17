#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const scenario = JSON.parse(readFileSync(join(process.env.CODEX_HOME, "scenario.json"), "utf8"));
const args = process.argv.slice(2);
if (args.includes("login") && args.includes("status")) {
  process.stderr.write(scenario.auth === "chatgpt" ? "Logged in using ChatGPT\n" : "Not logged in\n");
  process.exit(scenario.auth === "chatgpt" ? 0 : 1);
}
if (args.includes("exec")) {
  const output = args[args.indexOf("--output-last-message") + 1];
  const finish = () => {
    writeFileSync(output, scenario.mode === "invalid" ? "not-json" : JSON.stringify({ title: "Explicit fixture", summary: "Test response", warnings: [], operations: [] }));
    process.stdout.write('{"type":"item.completed","item":{"type":"agent_message"}}\n');
    process.exit(0);
  };
  if (scenario.mode === "tool") process.stdout.write('{"type":"item.started","item":{"type":"file_change"}}\n');
  if (scenario.mode === "late") {
    process.on("SIGTERM", finish);
    writeFileSync(join(process.env.CODEX_HOME, "started"), "ready");
    setInterval(() => {}, 1000);
  } else finish();
}
