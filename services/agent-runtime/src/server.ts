import { randomBytes } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { Auth } from "./auth.ts";
import { CodexTransport } from "./codex-transport.ts";
import { openAiSettings, parseOpenAiConfig } from "./config.ts";
import { createAdapter } from "./http.ts";
import { Jobs } from "./jobs.ts";
import { codexAgent, createVisionRuntime, ENGINE_VERSION } from "./model.ts";
import { CODEX_VERSION } from "./policy.ts";
import { NATIVE_CODEX, verifySafety } from "./safety.ts";
import { ServiceError, type ProviderStatus } from "./types.ts";

const home = "/auth";
const runs = "/tmp/longcut-runs";
const tokenPath = "/run/codex-link/token";
const [nodeMajor, nodeMinor] = process.versions.node.split(".").map(Number);
if ((nodeMajor ?? 0) < 24 || (nodeMajor === 24 && (nodeMinor ?? 0) < 22)) throw new ServiceError("NODE_VERSION", 503, "Agent runtime requires Node 24.22 or newer.");
await mkdir(runs, { recursive: true, mode: 0o700 });
await mkdir(home, { recursive: true, mode: 0o700 });
await mkdir("/run/codex-link", { recursive: true, mode: 0o700 });
try { await writeFile(tokenPath, randomBytes(32).toString("hex"), { flag: "wx", mode: 0o600 }); }
catch (error) { if (!(error instanceof Error && "code" in error && error.code === "EEXIST")) throw error; }
const token = Buffer.from((await readFile(tokenPath, "utf8")).trim());
if (token.length < 32) throw new ServiceError("INVALID_LINK", 500, "Invalid internal service credential.");

let codexAvailable = true;
let codexDetail: string | null = null;
try { await verifySafety(home, runs); }
catch (error) {
  codexAvailable = false;
  codexDetail = error instanceof ServiceError ? "Codex safety verification failed" : "Codex integration is unavailable";
}
const openAi = parseOpenAiConfig(openAiSettings(process.env));
const openAiRuntime = openAi.kind === "configured" ? createVisionRuntime(openAi.config) : null;
const auth = new Auth(NATIVE_CODEX, home, codexAvailable, codexDetail);
const codexTransport = new CodexTransport({ executable: NATIVE_CODEX, home, runs });
const jobs = new Jobs({
  auth,
  codex: codexAgent(codexTransport),
  codexTransport,
  openAi: openAiRuntime?.agent ?? null,
  openAiTransport: openAiRuntime?.transport ?? null,
});
const codexStatus = (): ProviderStatus => ({
  id: "codex", name: "ChatGPT subscription (Codex)", configured: codexAvailable,
  model: codexAvailable ? `codex-cli ${CODEX_VERSION}` : null, endpoint: null, detail: codexDetail,
});
const adapter = createAdapter({ auth, jobs, token, providers: () => [codexStatus(), openAi.status] });
adapter.server.listen(4100, "0.0.0.0", () => console.info(JSON.stringify({ event: "agent-runtime.listening", engine_version: ENGINE_VERSION })));
for (const signal of ["SIGTERM", "SIGINT"]) {
  process.on(signal, () => { void adapter.shutdown().catch(() => { process.exitCode = 1; }); });
}
