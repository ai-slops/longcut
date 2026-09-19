import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { createGateway } from "./gateway.mjs";

async function listen(server) {
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert(address && typeof address === "object");
  return `http://127.0.0.1:${address.port}`;
}

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), "longcut-gateway-"));
  await writeFile(join(root, "index.html"), "<!doctype html><title>Longcut</title>");
  const backend = createServer(async (req, res) => {
    if (req.headers.range === "bytes=0-3") {
      res.writeHead(206, { "content-type": "video/mp4", "content-range": "bytes 0-3/8", "accept-ranges": "bytes", "content-length": "4" });
      res.end("abcd");
      return;
    }
    let body = "";
    for await (const part of req) body += part;
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ method: req.method, path: req.url, body }));
  });
  const upstream = await listen(backend);
  const gateway = createGateway({ backend: upstream, webRoot: root });
  const url = await listen(gateway);
  t.after(async () => {
    for (const server of [gateway, backend]) {
      server.closeAllConnections();
      await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    }
    await rm(root, { recursive: true });
  });
  return { url };
}

test("serves built application when opening a workspace route", async (t) => {
  // Given a gateway with a built SPA.
  const { url } = await fixture(t);
  // When the user opens an application route.
  const result = await fetch(`${url}/project/example`);
  // Then the SPA document is served with security headers.
  assert.equal(result.status, 200);
  assert.match(result.headers.get("content-type"), /text\/html/);
  assert.equal(result.headers.get("x-content-type-options"), "nosniff");
});

test("preserves byte ranges when seeking media through the gateway", async (t) => {
  // Given an upstream serving range requests.
  const { url } = await fixture(t);
  // When the video player requests a byte range.
  const result = await fetch(`${url}/api/v1/media/clip`, { headers: { Range: "bytes=0-3" } });
  // Then both partial-content metadata and bytes survive the proxy.
  assert.equal(result.status, 206);
  assert.equal(result.headers.get("content-range"), "bytes 0-3/8");
  assert.equal(await result.text(), "abcd");
});

test("streams command payloads when their origin is the application", async (t) => {
  // Given the same-origin application.
  const { url } = await fixture(t);
  // When a command is submitted.
  const result = await fetch(`${url}/api/v1/projects/test/commands`, {
    method: "POST", headers: { Origin: url, "content-type": "application/json" }, body: '{"type":"undo"}',
  });
  // Then the backend receives the original method, path, and payload.
  assert.deepEqual(await result.json(), { method: "POST", path: "/api/v1/projects/test/commands", body: '{"type":"undo"}' });
});

test("rejects mutations when requested by an unrelated website", async (t) => {
  // Given the local application gateway.
  const { url } = await fixture(t);
  // When a third-party website attempts a mutation.
  const result = await fetch(`${url}/api/v1/projects`, { method: "POST", headers: { Origin: "https://untrusted.example" }, body: "{}" });
  // Then the request is rejected before reaching the core.
  assert.equal(result.status, 403);
  assert.equal((await result.json()).error.code, "ORIGIN_REJECTED");
});

test("rejects encoded traversal outside the web build directory", async (t) => {
  // Given the static asset boundary.
  const { url } = await fixture(t);
  // When an encoded slash attempts to escape that boundary.
  const result = await fetch(`${url}/..%2f..%2fetc/passwd`);
  // Then the filesystem path is not served.
  assert.equal(result.status, 403);
});

test("rejects malformed percent encodings with a client error", async (t) => {
  // Given the static asset boundary.
  const { url } = await fixture(t);
  // When a malformed URL is requested.
  const result = await fetch(`${url}/%GG`);
  // Then the response is actionable rather than a process crash.
  assert.equal(result.status, 400);
});
