import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fail, ok } from "@slop-lab/part/contracts/index.js";
import type { Contracts, Model } from "@slop-lab/part/reference/index.js";
import { EventGate, policyArgs } from "./policy.ts";
import { run, type Process } from "./process.ts";
import { JobRequestSchema, ProposalSchema, type JobRequest } from "./types.ts";

export type CodexTransportDependencies = {
  readonly mkdtemp: (prefix: string) => Promise<string>;
  readonly readFile: (path: string, encoding: "utf8") => Promise<string>;
  readonly rm: (path: string, options: { readonly recursive: true; readonly force: true }) => Promise<void>;
  readonly stat: (path: string) => Promise<{ readonly size: number }>;
  readonly writeFile: (path: string, data: string | Uint8Array, options: { readonly mode: number }) => Promise<void>;
  readonly launch: typeof run;
};
export type CodexTransportConfig = {
  readonly executable: string;
  readonly home: string;
  readonly runs: string;
  readonly dependencies?: CodexTransportDependencies;
};
type Attempt = {
  readonly cancellation: AbortController;
  readonly settled: Promise<void>;
  readonly settle: () => void;
  process: Process | null;
  directory: string | null;
};
const defaultDependencies: CodexTransportDependencies = {
  mkdtemp: (prefix) => mkdtemp(prefix),
  readFile: (path, encoding) => readFile(path, encoding),
  rm: (path, options) => rm(path, options),
  stat: (path) => stat(path),
  writeFile: (path, data, options) => writeFile(path, data, options),
  launch: run,
};

export class CodexTransport implements Model.Transport {
  readonly id = "longcut/codex-cli";
  private readonly active = new Map<string, Attempt>();
  private readonly config: CodexTransportConfig;
  private readonly dependencies: CodexTransportDependencies;
  private stopping = false;
  constructor(config: CodexTransportConfig) {
    this.config = config;
    this.dependencies = config.dependencies ?? defaultDependencies;
  }

  async cancel(id: string): Promise<void> {
    const attempt = this.active.get(id);
    attempt?.cancellation.abort();
    attempt?.process?.cancel();
    await attempt?.settled;
  }

  async shutdown(): Promise<void> {
    this.stopping = true;
    const attempts = [...this.active.values()];
    for (const attempt of attempts) {
      attempt.cancellation.abort();
      attempt.process?.cancel();
    }
    await Promise.all(attempts.map((attempt) => attempt.settled));
  }

  async send(request: Model.EncodedRequest, _profile: Model.Profile, _auth: Model.CredentialBinding, context: Parameters<Model.Transport["send"]>[3]) {
    let input: JobRequest;
    try { input = JobRequestSchema.parse(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(request.body))); }
    catch { return fail("codex.input", "Invalid Codex broker input"); }
    if (this.stopping) return fail("codex.stopping", "Codex transport is stopping", "cancelled", "not-attempted");
    if (this.active.has(input.id)) return fail("codex.active", "Codex transport attempt already exists", "conflict", "not-attempted");
    const completion = Promise.withResolvers<void>();
    const attempt: Attempt = { cancellation: new AbortController(), settled: completion.promise, settle: completion.resolve, process: null, directory: null };
    this.active.set(input.id, attempt);
    const cancel = () => { attempt.cancellation.abort(); attempt.process?.cancel(); };
    context.signal.addEventListener("abort", cancel, { once: true });
    if (context.signal.aborted) cancel();
    let result: Contracts.Result<Model.WireResponse>;
    try {
      result = await this.execute(input, attempt);
    } catch {
      result = fail("codex.failed", "Codex broker failed", attempt.cancellation.signal.aborted ? "cancelled" : "unavailable");
    }
    try {
      if (attempt.directory) await this.dependencies.rm(attempt.directory, { recursive: true, force: true });
    } catch {
      result = fail("codex.cleanup", "Codex broker cleanup failed", "unavailable", "unknown");
    } finally {
      context.signal.removeEventListener("abort", cancel);
      attempt.settle();
      this.active.delete(input.id);
    }
    return result;
  }

  private async execute(input: JobRequest, attempt: Attempt): Promise<Contracts.Result<Model.WireResponse>> {
    if (attempt.cancellation.signal.aborted) return fail("codex.cancelled", "Codex attempt was cancelled", "cancelled", "not-attempted");
    attempt.directory = await this.dependencies.mkdtemp(join(this.config.runs, "run-"));
    if (attempt.cancellation.signal.aborted) return fail("codex.cancelled", "Codex attempt was cancelled", "cancelled", "not-attempted");
    const images: string[] = [];
    for (const [index, image] of input.images.entries()) {
      const path = join(attempt.directory, `frame-${index + 1}.jpg`);
      await this.dependencies.writeFile(path, Buffer.from(image, "base64"), { mode: 0o600 });
      if (attempt.cancellation.signal.aborted) return fail("codex.cancelled", "Codex attempt was cancelled", "cancelled", "not-attempted");
      images.push(path);
    }
    const schemaPath = join(attempt.directory, "schema.json");
    const resultPath = join(attempt.directory, "result.json");
    await this.dependencies.writeFile(schemaPath, JSON.stringify(input.schema), { mode: 0o600 });
    if (attempt.cancellation.signal.aborted) return fail("codex.cancelled", "Codex attempt was cancelled", "cancelled", "not-attempted");
    const gate = new EventGate();
    attempt.process = this.dependencies.launch(this.config.executable, [
      ...policyArgs(attempt.directory, this.config.home), "exec", "--ignore-user-config", "--ignore-rules", "--strict-config",
      "--ephemeral", "--json", "--skip-git-repo-check", "--color", "never", "--output-schema", schemaPath,
      "--output-last-message", resultPath, ...images.flatMap((path) => ["--image", path]), "--", "-",
    ], { cwd: attempt.directory, home: this.config.home, timeoutMs: 600_000, input: input.prompt, onOutput: (stream, text) => { if (stream === "stdout") gate.accept(text); } });
    if (attempt.cancellation.signal.aborted) attempt.process.cancel();
    const exit = await attempt.process.done;
    if (exit.reason === "policy") return fail("codex.tool-attempt", "Codex attempted a tool or emitted an unsupported event");
    if (exit.reason !== "exit" || exit.code !== 0) return fail("codex.failed", "Codex did not complete", attempt.cancellation.signal.aborted ? "cancelled" : "unavailable");
    gate.finish();
    if ((await this.dependencies.stat(resultPath)).size > 262_144) return fail("codex.output-limit", "Codex response exceeded the output limit");
    const content = await this.dependencies.readFile(resultPath, "utf8");
    if (!ProposalSchema.safeParse(JSON.parse(content)).success) return fail("codex.proposal", "Codex returned an invalid proposal");
    const body = new TextEncoder().encode(JSON.stringify({ choices: [{ index: 0, finish_reason: "stop", message: { content } }] }));
    return ok({ status: 200, headers: { "content-type": "application/json" }, body: (async function* () { yield body; })() });
  }
}
