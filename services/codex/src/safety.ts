import { createHash, randomUUID } from "node:crypto";
import { access, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { CODEX_VERSION, DISABLED_FEATURES, policyArgs } from "./policy.ts";
import { run } from "./process.ts";
import { ServiceError } from "./types.ts";

export const NATIVE_CODEX = "/usr/local/lib/node_modules/@openai/codex/node_modules/@openai/codex-linux-x64/vendor/x86_64-unknown-linux-musl/bin/codex";
const EXPECTED_SHA256 = "0753dfe1d8b87a52436deb13eb1c549661ef4c84fee2c5aa688385eebeccb761";

export async function verifySafety(home: string, runs: string): Promise<string> {
  const hash = createHash("sha256").update(await readFile(NATIVE_CODEX)).digest("hex");
  if (hash !== EXPECTED_SHA256) throw new ServiceError("BINARY_CHANGED", 503, "Codex binary digest does not match the reviewed version.");
  const version = await run(NATIVE_CODEX, ["--version"], { cwd: runs, home, timeoutMs: 15_000 }).done;
  if (version.code !== 0 || version.stdout.trim() !== `codex-cli ${CODEX_VERSION}`) throw new ServiceError("VERSION_CHANGED", 503, "Unsupported Codex version.");
  const directory = await mkdtemp(join(runs, "canary-"));
  const canary = join(home, `canary-${randomUUID()}`);
  const forbiddenWrite = join(directory, "forbidden-write");
  const sentinel = randomUUID();
  await writeFile(canary, sentinel, { mode: 0o600 });
  try {
    const args = policyArgs(directory, home);
    const features = await run(NATIVE_CODEX, [...args, "features", "list"], { cwd: directory, home, timeoutMs: 15_000 }).done;
    if (features.code !== 0) throw new ServiceError("POLICY_INVALID", 503, "Codex rejected the restricted configuration.");
    for (const feature of DISABLED_FEATURES) {
      const line = features.stdout.split("\n").find((item) => item.startsWith(`${feature} `));
      if (!line || line.trim().split(/\s+/).at(-1) !== "false") throw new ServiceError("POLICY_INVALID", 503, "A required Codex tool feature was not disabled.");
    }
    const probe = async (command: readonly string[]) => run(NATIVE_CODEX, [...args, "sandbox", "linux", "--", ...command], { cwd: directory, home, timeoutMs: 15_000 }).done;
    const positive = await probe(["/usr/bin/true"]);
    const unavailable = /bwrap: No permissions to create new namespace/.test(positive.stderr);
    if (positive.code !== 0 && !unavailable) throw new ServiceError("POLICY_INVALID", 503, "Codex sandbox initialization failed; AI editing is disabled.");
    await symlink(canary, join(directory, "alias"));
    for (const path of [canary, join(directory, "alias"), "/proc/self/environ"]) {
      const denied = await probe(["/bin/cat", path]);
      if (denied.code === 0 || denied.stdout.includes(sentinel)) throw new ServiceError("ISOLATION_FAILED", 503, "Credential read isolation failed; AI editing is disabled.");
    }
    const write = await probe(["/bin/sh", "-c", 'printf forbidden > "$1"', "canary", forbiddenWrite]);
    let exists = true;
    try { await access(forbiddenWrite); } catch (error) {
      if (error instanceof Error && "code" in error && error.code === "ENOENT") exists = false;
      else throw error;
    }
    if (write.code === 0 || exists) throw new ServiceError("ISOLATION_FAILED", 503, "Filesystem write isolation failed; AI editing is disabled.");
    return unavailable ? "tools-denied; tool-free inference requires account authorization" : "restricted; no tool results accepted";
  } finally {
    await rm(canary, { force: true });
    await rm(directory, { recursive: true, force: true });
  }
}
