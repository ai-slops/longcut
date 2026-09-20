import { timingSafeEqual } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { z } from "zod";
import type { Auth } from "./auth.ts";
import type { Jobs } from "./jobs.ts";
import { CODEX_VERSION } from "./policy.ts";
import { JobRequestSchema, ServiceError } from "./types.ts";

type Services = {
  readonly auth: Pick<Auth, "cached" | "status" | "pending" | "login" | "cancel" | "logout" | "shutdown">;
  readonly jobs: Pick<Jobs, "busy" | "get" | "start" | "cancel" | "shutdown">;
  readonly token: Buffer;
  readonly safety: string;
};
const MAX_BODY = 16 * 1024 * 1024;
async function body(request: IncomingMessage): Promise<unknown> {
  if (Number(request.headers["content-length"]) > MAX_BODY) {
    request.resume();
    throw new ServiceError("BODY_TOO_LARGE", 413, "AI context exceeds the payload limit.");
  }
  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of request.iterator({ destroyOnReturn: false })) {
    length += chunk.length;
    if (length > MAX_BODY) {
      request.resume();
      throw new ServiceError("BODY_TOO_LARGE", 413, "AI context exceeds the payload limit.");
    }
    chunks.push(chunk);
  }
  try { return JSON.parse(Buffer.concat(chunks).toString("utf8")); }
  catch (error) { if (error instanceof SyntaxError) throw new ServiceError("INVALID_JSON", 422, "Invalid JSON body."); throw error; }
}
function send(response: ServerResponse, status: number, value: unknown): void {
  if (response.destroyed) return;
  response.writeHead(status, { "content-type": "application/json", "cache-control": "no-store", "x-content-type-options": "nosniff" });
  response.end(JSON.stringify(value));
}

export function createAdapter({ auth, jobs, token, safety }: Services) {
  let accountOperation = false;
  let submission = false;
  let stopping = false;
  const server = createServer(async (request, response) => {
    try {
      if (stopping) throw new ServiceError("SHUTTING_DOWN", 503, "Codex is shutting down.");
      const path = new URL(request.url ?? "/", "http://localhost").pathname;
      if (request.method === "GET" && path === "/health") return send(response, 200, { status: "ok", version: CODEX_VERSION, safety });
      const header = request.headers.authorization ?? "";
      if (!header.startsWith("Bearer ")) throw new ServiceError("UNAUTHORIZED", 401, "Unauthorized internal client.");
      const supplied = Buffer.from(header.slice(7));
      if (supplied.length !== token.length || !timingSafeEqual(supplied, token)) throw new ServiceError("UNAUTHORIZED", 401, "Unauthorized internal client.");
      if (request.method === "GET" && path === "/status") {
        const status = jobs.busy() || accountOperation || submission ? auth.cached() : await auth.status();
        return send(response, 200, { ...status, state: jobs.busy() || submission ? "busy" : status.state });
      }
      if (request.method === "POST" && ["/login", "/login/cancel", "/logout"].includes(path)) {
        if (jobs.busy() || accountOperation || submission) throw new ServiceError("CODEX_BUSY", 409, "Another account operation or AI run is active.");
        accountOperation = true;
        try {
          switch (path) {
            case "/login": return send(response, 200, await auth.login());
            case "/login/cancel": return send(response, 200, await auth.cancel());
            case "/logout": return send(response, 200, await auth.logout());
          }
        } finally { accountOperation = false; }
      }
      if (request.method === "POST" && path === "/jobs") {
        if (accountOperation || submission || jobs.busy() || auth.pending()) throw new ServiceError("CODEX_BUSY", 409, "Another account operation or AI run is active.");
        submission = true;
        try {
          const input = JobRequestSchema.parse(await body(request));
          if (stopping) throw new ServiceError("SHUTTING_DOWN", 503, "Codex is shutting down.");
          return send(response, 202, await jobs.start(input));
        } finally { submission = false; }
      }
      const match = /^\/jobs\/([a-fA-F0-9-]{36})(\/cancel)?$/.exec(path);
      const id = match?.[1]?.toLowerCase();
      if (id && request.method === "GET" && !match?.[2]) return send(response, 200, jobs.get(id));
      if (id && request.method === "POST" && match?.[2]) return send(response, 200, await jobs.cancel(id));
      throw new ServiceError("NOT_FOUND", 404, "Endpoint not found.");
    } catch (error) {
      if (error instanceof ServiceError) send(response, error.status, { error: { code: error.code, message: error.message } });
      else if (error instanceof z.ZodError) send(response, 422, { error: { code: "INVALID_REQUEST", message: "Invalid AI request shape." } });
      else send(response, 500, { error: { code: "INTERNAL_ERROR", message: "Codex service could not complete this request." } });
    }
  });
  server.requestTimeout = 60_000;
  return {
    server,
    shutdown: async () => {
      stopping = true;
      server.close();
      await Promise.all([jobs.shutdown(), auth.shutdown()]);
      server.closeAllConnections();
    },
  };
}
