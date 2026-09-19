import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer, request } from "node:http";
import { extname, resolve, sep } from "node:path";

const mime = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".svg", "image/svg+xml"],
  [".png", "image/png"],
  [".ico", "image/x-icon"],
  [".woff2", "font/woff2"],
]);

function fail(response, status, code, message) {
  if (response.headersSent) {
    response.destroy();
    return;
  }
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(JSON.stringify({ error: { code, message } }));
}

function forward(incoming, response, backend) {
  const headers = { ...incoming.headers, host: backend.host };
  for (const name of ["connection", "keep-alive", "proxy-authorization", "proxy-authenticate", "upgrade", "te", "trailer", "transfer-encoding"]) {
    delete headers[name];
  }
  const upstream = request(backend, { method: incoming.method, path: incoming.url, headers }, (result) => {
    const outgoing = { ...result.headers };
    delete outgoing["connection"];
    delete outgoing["transfer-encoding"];
    response.writeHead(result.statusCode ?? 502, outgoing);
    result.on("error", () => response.destroy());
    result.pipe(response);
  });
  upstream.setTimeout(30 * 60 * 1000, () => upstream.destroy(new Error("Backend request timed out")));
  upstream.on("error", () => fail(response, 502, "BACKEND_UNAVAILABLE", "The Longcut core is unavailable. Start the backend and try again."));
  incoming.on("aborted", () => upstream.destroy());
  response.on("close", () => upstream.destroy());
  incoming.pipe(upstream);
}

/** Same-origin streaming gateway; caller owns listen and shutdown. */
export function createGateway({ backend, webRoot }) {
  const upstream = new URL(backend);
  if (upstream.protocol !== "http:") throw new Error("LONGCUT_BACKEND must use internal HTTP");
  const root = resolve(webRoot);
  return createServer(async (incoming, response) => {
    response.setHeader("x-content-type-options", "nosniff");
    response.setHeader("referrer-policy", "same-origin");
    response.setHeader("content-security-policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; media-src 'self' blob:; connect-src 'self'; font-src 'self'; frame-ancestors 'self'; base-uri 'self'; form-action 'self'");
    const mutation = !["GET", "HEAD", "OPTIONS"].includes(incoming.method ?? "GET");
    if (mutation) {
      let originHost;
      try { originHost = incoming.headers.origin ? new URL(incoming.headers.origin).host : undefined; }
      catch { return fail(response, 403, "ORIGIN_REJECTED", "Invalid request origin."); }
      if (incoming.headers["sec-fetch-site"] === "cross-site" || (originHost && originHost !== incoming.headers.host)) {
        return fail(response, 403, "ORIGIN_REJECTED", "Cross-origin changes are not permitted.");
      }
    }
    if (incoming.url?.startsWith("/api/v1/")) return forward(incoming, response, upstream);
    if (!["GET", "HEAD"].includes(incoming.method ?? "GET")) return fail(response, 405, "METHOD_NOT_ALLOWED", "Use the command API for changes.");
    try {
      const pathname = decodeURIComponent(new URL(incoming.url ?? "/", "http://localhost").pathname);
      const file = resolve(root, `.${pathname === "/" ? "/index.html" : pathname}`);
      if (!file.startsWith(`${root}${sep}`)) return fail(response, 403, "PATH_REJECTED", "Invalid asset path.");
      const asset = extname(file) ? file : resolve(root, "index.html");
      const metadata = await stat(asset);
      if (!metadata.isFile()) return fail(response, 404, "NOT_FOUND", "Asset not found.");
      response.writeHead(200, {
        "content-type": mime.get(extname(asset)) ?? "application/octet-stream",
        "content-length": metadata.size,
        "cache-control": asset.includes(`${sep}assets${sep}`) ? "public, max-age=31536000, immutable" : "no-cache",
      });
      if (incoming.method === "HEAD") return response.end();
      createReadStream(asset).on("error", () => response.destroy()).pipe(response);
    } catch (error) {
      if (error instanceof URIError) return fail(response, 400, "INVALID_URL", "Malformed URL encoding.");
      if (error instanceof Error && "code" in error && error.code === "ENOENT") {
        return fail(response, 404, "NOT_FOUND", "Build the web application before opening Longcut.");
      }
      fail(response, 500, "GATEWAY_ERROR", "Unable to serve the application.");
    }
  });
}
