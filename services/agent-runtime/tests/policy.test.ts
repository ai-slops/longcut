import assert from "node:assert/strict";
import test from "node:test";
import { deviceChallenge } from "../src/auth.ts";
import { DISABLED_FEATURES, EventGate, policyArgs } from "../src/policy.ts";
import { JobRequestSchema, ProposalSchema } from "../src/types.ts";

test("Codex policy denies credentials and every reviewed tool feature", () => {
  const args = policyArgs("/tmp/run", "/auth");
  assert(args.includes('permissions.longcut.filesystem={":minimal"="read","/tmp/run"="read","/auth"="deny","/run/codex-link"="deny","/proc"="deny"}'));
  assert(args.includes("permissions.longcut.network.enabled=false"));
  assert(args.includes('forced_login_method="chatgpt"'));
  assert(!args.includes("--sandbox"));
  assert(!args.includes("--dangerously-bypass-approvals-and-sandbox"));
  for (const feature of DISABLED_FEATURES) assert(args.includes(feature));
});

test("tool-free JSONL accepts split chunks and a final unterminated line", () => {
  const gate = new EventGate();
  gate.accept('{"type":"thread.started"}\n{"type":"item.com');
  gate.accept('pleted","item":{"type":"agent_message"}}');
  assert.doesNotThrow(() => gate.finish());
});

for (const kind of ["command_execution", "file_change", "mcp_tool_call", "web_search", "image_generation", "plan"]) {
  test(`a ${kind} event invalidates the proposal`, () => {
    assert.throws(() => new EventGate().accept(`${JSON.stringify({ type: "item.started", item: { type: kind } })}\n`));
  });
}

test("unknown events, invented operations, and execution overrides are rejected", () => {
  assert.throws(() => new EventGate().accept('{"type":"new.capability"}\n'));
  assert.equal(ProposalSchema.safeParse({ title: "Edit", summary: "", warnings: [], operations: [{ type: "shell", command: "rm" }] }).success, false);
  assert.equal(JobRequestSchema.safeParse({ id: crypto.randomUUID(), prompt: "Edit", schema: {}, images: [], executable: "/bin/sh" }).success, false);
});

test("login exposes only the official device endpoint and bounded code", () => {
  const expiry = "2026-09-20T00:10:00Z";
  assert.deepEqual(deviceChallenge("Visit https://auth.openai.com/codex/device\nCode: ABCD-12345", expiry), { url: "https://auth.openai.com/codex/device", code: "ABCD-12345", expires_at: expiry });
  assert.equal(deviceChallenge("https://attacker.example/codex/device ABCD-12345", expiry), null);
});
