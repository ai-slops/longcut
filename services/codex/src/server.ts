import { randomBytes } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { Auth } from "./auth.ts";
import { createAdapter } from "./http.ts";
import { Jobs } from "./jobs.ts";
import { CODEX_VERSION } from "./policy.ts";
import { NATIVE_CODEX, verifySafety } from "./safety.ts";
import { ServiceError } from "./types.ts";

const home = "/auth";
const runs = "/tmp/longcut-runs";
const tokenPath = "/run/codex-link/token";
await mkdir(runs, { recursive: true, mode: 0o700 });
await mkdir(home, { recursive: true, mode: 0o700 });
await mkdir("/run/codex-link", { recursive: true, mode: 0o700 });
try { await writeFile(tokenPath, randomBytes(32).toString("hex"), { flag: "wx", mode: 0o600 }); }
catch (error) { if (!(error instanceof Error && "code" in error && error.code === "EEXIST")) throw error; }
const token = Buffer.from((await readFile(tokenPath, "utf8")).trim());
if (token.length < 32) throw new ServiceError("INVALID_LINK", 500, "Invalid internal service credential.");
const safety = await verifySafety(home, runs);
const auth = new Auth(NATIVE_CODEX, home);
const jobs = new Jobs({ executable: NATIVE_CODEX, home, runs, auth });
const adapter = createAdapter({ auth, jobs, token, safety });
adapter.server.listen(4100, "0.0.0.0", () => console.info(JSON.stringify({ event: "codex.listening", version: CODEX_VERSION, safety })));
for (const signal of ["SIGTERM", "SIGINT"]) {
  process.on(signal, () => { void adapter.shutdown().catch(() => { process.exitCode = 1; }); });
}
