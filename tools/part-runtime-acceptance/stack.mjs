import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { chmod, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { join } from "node:path";
import { Auth } from "/opt/longcut-agent-runtime/src/auth.ts";
import { CodexTransport } from "/opt/longcut-agent-runtime/src/codex-transport.ts";
import { createAdapter } from "/opt/longcut-agent-runtime/src/http.ts";
import { Jobs } from "/opt/longcut-agent-runtime/src/jobs.ts";
import { codexAgent, createVisionRuntime, ENGINE_VERSION } from "/opt/longcut-agent-runtime/src/model.ts";
import { CODEX_VERSION } from "/opt/longcut-agent-runtime/src/policy.ts";
import { inspectJpegBase64, proposalFor, sourceFrames } from "./fixture-contract.mjs";

const root = "/acceptance/state";
const home = join(root, "auth");
const runs = join(root, "runs");
const scenarioPath = join(home, "scenario.json");
const codexCapturesPath = join(home, "codex-captures.jsonl");
const tokenPath = "/run/codex-link/token";
await mkdir(home, { recursive: true, mode: 0o700 });
await mkdir(runs, { recursive: true, mode: 0o700 });
await mkdir("/run/codex-link", { recursive: true, mode: 0o700 });
await chmod("/acceptance/fake-codex.mjs", 0o700);

async function setScenario(value) {
  const temporary = `${scenarioPath}.next`;
  await writeFile(temporary, JSON.stringify(value), { mode: 0o600 });
  await rename(temporary, scenarioPath);
}
await setScenario({ mode: "ready", release: false, response: "json" });
await writeFile(codexCapturesPath, "", { mode: 0o600 });
const token = randomBytes(32).toString("hex");
await writeFile(tokenPath, token, { mode: 0o600 });

let httpCaptures = [];
let activeHttp = 0;
let releaseHttp = Promise.withResolvers();
const fixture = createServer(async (request, response) => {
  const url = new URL(request.url ?? "/", "http://fixture");
  if (request.method === "POST" && url.pathname === "/control/scenario") {
    const value = JSON.parse(await readBody(request));
    await setScenario({ ...value, release: false });
    httpCaptures = [];
    releaseHttp = Promise.withResolvers();
    return json(response, 200, { configured: true, test_provider: true });
  }
  if (request.method === "POST" && url.pathname === "/control/release") {
    const scenario = JSON.parse(await readFile(scenarioPath, "utf8"));
    await setScenario({ ...scenario, release: true });
    releaseHttp.resolve();
    return json(response, 200, { released: true });
  }
  if (request.method === "GET" && url.pathname === "/control/state") {
    const codex = (await readFile(codexCapturesPath, "utf8")).trim().split("\n").filter(Boolean).map(JSON.parse);
    return json(response, 200, { test_provider: true, active_http: activeHttp, http: httpCaptures, codex });
  }
  assert.equal(url.pathname, "/v1/chat/completions");
  assert.equal(request.headers.authorization, "Bearer fixture-only-key");
  const body = JSON.parse(await readBody(request));
  const content = body.messages[0].content;
  const prompt = content[0].text;
  const images = content.slice(1);
  for (const image of images) inspectJpegBase64(image.image_url.url.replace("data:image/jpeg;base64,", ""));
  assert.equal(body.response_format.json_schema.strict, true);
  httpCaptures.push({
    provider: "openai-compatible",
    test_provider: true,
    bearer_auth_valid: true,
    image_count: images.length,
    response_format: body.response_format.type,
    source_frames: sourceFrames(prompt),
  });
  activeHttp++;
  const closed = Promise.withResolvers();
  response.once("close", closed.resolve);
  const scenario = JSON.parse(await readFile(scenarioPath, "utf8"));
  if (scenario.mode === "hold" && scenario.release !== true) await Promise.race([releaseHttp.promise, closed.promise]);
  activeHttp--;
  if (response.destroyed) return;
  const proposal = JSON.stringify(proposalFor(prompt));
  if (scenario.response === "sse") {
    response.writeHead(200, { "content-type": "text/event-stream" });
    response.end(`data: ${JSON.stringify({ choices: [{ index: 0, delta: { content: proposal }, finish_reason: "stop" }] })}\n\ndata: [DONE]\n\n`);
  } else {
    json(response, 200, { choices: [{ index: 0, finish_reason: "stop", message: { content: proposal } }] });
  }
});

async function readBody(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}
function json(response, status, value) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(value));
}
fixture.listen(4111, "0.0.0.0");

const auth = new Auth("/acceptance/fake-codex.mjs", home, true, null);
const codexTransport = new CodexTransport({ executable: "/acceptance/fake-codex.mjs", home, runs });
const openAi = createVisionRuntime({
  key: "fixture-only-key",
  endpoint: "http://127.0.0.1:4111/v1/chat/completions",
  origin: "http://127.0.0.1:4111",
  model: "fixture-vision",
  responseFormat: "json_schema",
  maxOutputTokens: 1024,
  tokenParameter: "max_completion_tokens",
});
const jobs = new Jobs({ auth, codex: codexAgent(codexTransport), codexTransport, openAi: openAi.agent, openAiTransport: openAi.transport });
const adapter = createAdapter({
  auth,
  jobs,
  token: Buffer.from(token),
  providers: () => [
    { id: "codex", name: "Acceptance fake Codex CLI", configured: true, model: `fake codex-cli ${CODEX_VERSION}`, endpoint: null, detail: "test_provider=true; no ChatGPT connection" },
    { id: "openai-compatible", name: "Acceptance loopback HTTP", configured: true, model: "fixture-vision", endpoint: "http://127.0.0.1:4111/v1/chat/completions", detail: "test_provider=true; local fixture" },
  ],
});
adapter.server.listen(4110, "0.0.0.0", () => process.stdout.write(`${JSON.stringify({ event: "acceptance-runtime.listening", engine_version: ENGINE_VERSION, test_provider: true })}\n`));

async function shutdown() {
  fixture.close();
  await adapter.shutdown();
}
for (const signal of ["SIGTERM", "SIGINT"]) process.on(signal, () => { void shutdown(); });
