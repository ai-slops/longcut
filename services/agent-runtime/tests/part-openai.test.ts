import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import { localContext, ok } from "@slop-lab/part/contracts/index.js";
import type { Model } from "@slop-lab/part/reference/index.js";
import { BoundedFetchTransport } from "../src/fetch-transport.ts";
import { partInput, visionAgent } from "../src/model.ts";
import type { OpenAiConfig } from "../src/config.ts";

const proposal = { title: "Fixture", summary: "Safe result", warnings: [], operations: [] };
const request = (id = crypto.randomUUID()) => ({ id, provider: "openai-compatible" as const, prompt: "structured context", schema: { type: "object" }, images: ["/9j/2Q=="] });
const remainsPendingForTurn = async (promise: Promise<unknown>): Promise<boolean> => Promise.race([
  promise.then(() => false),
  new Promise<boolean>((resolve) => setImmediate(() => resolve(true))),
]);

async function fixture(t: TestContext, response: (body: string) => { readonly status?: number; readonly type?: string; readonly location?: string; readonly chunks: readonly Uint8Array[] }) {
  let capturedBody = "";
  let authorization = "";
  const server = createServer(async (incoming, outgoing) => {
    authorization = incoming.headers.authorization ?? "";
    const chunks: Buffer[] = [];
    for await (const chunk of incoming) chunks.push(chunk);
    capturedBody = Buffer.concat(chunks).toString("utf8");
    const value = response(capturedBody);
    outgoing.writeHead(value.status ?? 200, { "content-type": value.type ?? "application/json", ...(value.location ? { location: value.location } : {}) });
    for (const chunk of value.chunks) outgoing.write(chunk);
    outgoing.end();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert(address && typeof address === "object");
  t.after(() => server.close());
  const endpoint = `http://127.0.0.1:${address.port}/v1/chat/completions`;
  const config: OpenAiConfig = { key: "fixture-key", endpoint, origin: new URL(endpoint).origin, model: "vision-fixture", responseFormat: "json_schema", maxOutputTokens: 1024, tokenParameter: "max_completion_tokens" };
  return { agent: visionAgent(config), captured: () => ({ body: JSON.parse(capturedBody), authorization }) };
}

test("real PART sends the legacy vision body and returns its execution ID", async (t) => {
  const app = await fixture(t, () => ({ chunks: [new TextEncoder().encode(JSON.stringify({ choices: [{ index: 0, finish_reason: "stop", message: { content: JSON.stringify(proposal) } }] }))] }));
  const started = await app.agent.start(await partInput(request()));
  assert.equal(started.ok, true);
  if (!started.ok) return;
  assert.match(started.value.ref.id, /^[0-9a-f-]{36}$/);
  const outcome = await started.value.done;
  assert.equal(outcome.phase, "completed");
  const captured = app.captured();
  assert.equal(captured.authorization, "Bearer fixture-key");
  assert.equal(captured.body.model, "vision-fixture");
  assert.equal(captured.body.stream, true);
  assert.equal(captured.body.max_completion_tokens, 1024);
  assert.equal(captured.body.messages[0].content[1].image_url.url, "data:image/jpeg;base64,/9j/2Q==");
  assert.equal(captured.body.response_format.json_schema.strict, true);
  await app.agent.dispose();
});

test("real PART decodes split SSE UTF-8 and rejects malformed terminal semantics", async (t) => {
  const encoder = new TextEncoder();
  const content = JSON.stringify(proposal);
  const valid = `data: ${JSON.stringify({ choices: [{ index: 0, delta: { content }, finish_reason: "stop" }] })}\n\ndata: [DONE]\n\n`;
  const bytes = encoder.encode(valid);
  const good = await fixture(t, () => ({ type: "text/event-stream", chunks: [bytes.slice(0, 17), bytes.slice(17)] }));
  const completed = await good.agent.generate(await partInput(request()));
  assert.equal(completed.ok && completed.value.phase, "completed");
  await good.agent.dispose();

  const bad = await fixture(t, () => ({ type: "text/event-stream", chunks: [encoder.encode(`data: ${JSON.stringify({ choices: [{ index: 0, delta: { content }, finish_reason: "length" }] })}\n\ndata: [DONE]\n\n`)] }));
  const rejected = await bad.agent.generate(await partInput(request()));
  assert.equal(rejected.ok && rejected.value.phase, "failed");
  await bad.agent.dispose();
});

test("real PART rejects provider errors, tool calls, malformed UTF-8, and oversized streams", async (t) => {
  const cases = [
    new TextEncoder().encode(JSON.stringify({ error: { message: "secret vendor detail" }, choices: [{ index: 0, finish_reason: "stop", message: { content: JSON.stringify(proposal) } }] })),
    new TextEncoder().encode(JSON.stringify({ choices: [{ index: 0, finish_reason: "stop", message: { content: JSON.stringify(proposal), tool_calls: [{ id: "call", type: "function", function: { name: "shell", arguments: "{}" } }] } }] })),
    Uint8Array.from([0xff, 0xfe]),
    new TextEncoder().encode("x".repeat(262_145)),
  ];
  for (const body of cases) {
    const app = await fixture(t, () => ({ chunks: [body] }));
    const outcome = await app.agent.generate(await partInput(request()));
    assert.equal(outcome.ok && outcome.value.phase, "failed");
    await app.agent.dispose();
  }
});

test("PART fetch transport rejects redirects without forwarding authorization", async (t) => {
  let sinkHits = 0;
  const sink = createServer((_request, response) => { sinkHits++; response.end(); });
  sink.listen(0, "127.0.0.1");
  await once(sink, "listening");
  const sinkAddress = sink.address();
  assert(sinkAddress && typeof sinkAddress === "object");
  t.after(() => sink.close());
  const app = await fixture(t, () => ({ status: 307, location: `http://127.0.0.1:${sinkAddress.port}/sink`, chunks: [new Uint8Array()] }));
  const outcome = await app.agent.generate(await partInput(request()));
  assert.equal(outcome.ok && outcome.value.phase, "failed");
  assert.equal(sinkHits, 0);
  await app.agent.dispose();
});

test("bounded fetch cancellation owns body cleanup beyond PART grace", async () => {
  const bodyEntered = Promise.withResolvers<void>();
  const bodyRelease = Promise.withResolvers<void>();
  const cleanupEntered = Promise.withResolvers<void>();
  const cleanupRelease = Promise.withResolvers<void>();
  const base: Model.Transport = {
    id: "fixture/fetch",
    async send(_request, _profile, _auth, context) {
      return ok({
        status: 200,
        headers: { "content-type": "application/json" },
        body: (async function* () {
          bodyEntered.resolve();
          try {
            await Promise.race([
              bodyRelease.promise,
              new Promise<void>((resolve) => context.signal.addEventListener("abort", () => resolve(), { once: true })),
            ]);
          } finally {
            cleanupEntered.resolve();
            await cleanupRelease.promise;
          }
        })(),
      });
    },
  };
  const transport = new BoundedFetchTransport(base);
  const jobId = crypto.randomUUID();
  const sent = transport.send(
    { protocol: "fixture", mediaType: "application/json", body: new Uint8Array(), digest: "fixture", metadata: { jobId }, inspectionBoundary: "outgoing-body" },
    { id: "fixture", revision: "1", adapterId: "fixture", transportId: transport.id, authStrategyId: "none", model: "fixture", defaults: {}, capabilities: { revision: "1", entries: [] } },
    { strategy: "none", material: undefined },
    localContext(),
  );
  await bodyEntered.promise;
  const cleanup = transport.cancel(jobId);
  try {
    assert.equal(await remainsPendingForTurn(cleanup), true);
  } finally {
    bodyRelease.resolve();
    await cleanupEntered.promise;
    cleanupRelease.resolve();
  }
  await cleanup;
  await sent;
});
