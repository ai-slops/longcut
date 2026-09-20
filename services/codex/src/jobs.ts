import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Auth } from "./auth.ts";
import { EventGate, policyArgs } from "./policy.ts";
import { run, type Process } from "./process.ts";
import { ProposalSchema, ServiceError, type Job, type JobRequest } from "./types.ts";

type JobConfig = { readonly executable: string; readonly home: string; readonly runs: string; readonly auth: Auth };
export class Jobs {
  private readonly records = new Map<string, Job>();
  private activeId: string | null = null;
  private process: Process | null = null;
  private completion: Promise<void> | null = null;
  private pendingStart: Promise<Job> | null = null;
  private stopping = false;
  private readonly config: JobConfig;
  constructor(config: JobConfig) { this.config = config; }
  busy(): boolean { return this.activeId !== null; }
  get(id: string): Job {
    const value = this.records.get(id);
    if (!value) throw new ServiceError("NOT_FOUND", 404, "AI job not found; it may have been interrupted by a restart.");
    return value;
  }
  async start(request: JobRequest): Promise<Job> {
    if (this.stopping) throw new ServiceError("SHUTTING_DOWN", 503, "Codex is shutting down.");
    if (this.records.has(request.id)) throw new ServiceError("DUPLICATE_JOB", 409, "Job ID already exists.");
    if (this.busy() || this.config.auth.pending()) throw new ServiceError("CODEX_BUSY", 409, "Another account operation or AI run is active.");
    this.activeId = request.id;
    this.pendingStart = this.authorize(request);
    try { return await this.pendingStart; }
    catch (error) { this.activeId = null; throw error; }
    finally { this.pendingStart = null; }
  }
  private async authorize(request: JobRequest): Promise<Job> {
    try {
      const status = await this.config.auth.status();
      if (this.stopping) throw new ServiceError("SHUTTING_DOWN", 503, "Codex is shutting down.");
      if (status.auth_mode !== "chatgpt") throw new ServiceError("AUTH_REQUIRED", 401, "Connect your ChatGPT account before generating an edit.");
      const job: Job = { id: request.id, status: "running", result: null, error: null };
      this.records.set(job.id, job);
      this.completion = this.execute(request);
      return job;
    } catch (error) { this.activeId = null; throw error; }
  }
  async cancel(id: string): Promise<Job> {
    const value = this.get(id);
    if (value.status !== "running") return value;
    const cancelled: Job = { ...value, status: "cancelled", result: null, error: null };
    this.records.set(id, cancelled);
    this.process?.cancel();
    return cancelled;
  }
  async waitForIdle(): Promise<void> { await this.completion; }
  async shutdown(): Promise<void> {
    this.stopping = true;
    try { await this.pendingStart; }
    catch (error) { if (!(error instanceof ServiceError)) throw error; }
    if (this.activeId && this.records.has(this.activeId)) await this.cancel(this.activeId);
    await this.waitForIdle();
  }
  private async execute(request: JobRequest): Promise<void> {
    let directory: string | null = null;
    try {
      directory = await mkdtemp(join(this.config.runs, "run-"));
      const images: string[] = [];
      for (const [index, image] of request.images.entries()) {
        const bytes = Buffer.from(image, "base64");
        if (bytes[0] !== 0xff || bytes[1] !== 0xd8 || bytes[2] !== 0xff) throw new ServiceError("INVALID_IMAGE", 422, "Only re-encoded JPEG samples are accepted.");
        const path = join(directory, `frame-${index + 1}.jpg`);
        await writeFile(path, bytes, { mode: 0o600 });
        images.push(path);
      }
      const schema = join(directory, "schema.json");
      const resultPath = join(directory, "result.json");
      await writeFile(schema, JSON.stringify(request.schema), { mode: 0o600 });
      if (this.get(request.id).status === "cancelled") return;
      const gate = new EventGate();
      const child = run(this.config.executable, [
        ...policyArgs(directory, this.config.home), "exec", "--ignore-user-config", "--ignore-rules", "--strict-config",
        "--ephemeral", "--json", "--skip-git-repo-check", "--color", "never",
        "--output-schema", schema, "--output-last-message", resultPath,
        ...images.flatMap((path) => ["--image", path]), "--", "-",
      ], {
        cwd: directory, home: this.config.home, timeoutMs: 600_000, input: request.prompt,
        onOutput: (stream, text) => { if (stream === "stdout") gate.accept(text); },
      });
      this.process = child;
      const exit = await child.done;
      if (this.get(request.id).status === "cancelled") return;
      if (exit.reason === "policy") throw new ServiceError("TOOL_ATTEMPT", 422, "Codex attempted a tool or emitted an unsupported event. The entire proposal was discarded; ask for a direct editing proposal without tools.");
      if (exit.reason !== "exit" || exit.code !== 0) {
        const usage = /usage limit|rate limit|quota|credits/i.test(exit.stderr + exit.stdout);
        throw new ServiceError("CODEX_FAILED", 502, usage ? "Your Codex usage limit was reached. Check your ChatGPT plan and retry later." : `Codex did not complete (${exit.reason}). Reconnect your account or retry a smaller request. No edits were applied.`);
      }
      gate.finish();
      const metadata = await stat(resultPath);
      if (metadata.size > 262_144) throw new ServiceError("OUTPUT_LIMIT", 422, "Codex returned too much data. Try a smaller scope.");
      const result = ProposalSchema.parse(JSON.parse(await readFile(resultPath, "utf8")));
      if (this.get(request.id).status !== "running") return;
      this.records.set(request.id, { id: request.id, status: "succeeded", result, error: null });
    } catch (error) {
      if (this.records.get(request.id)?.status !== "cancelled") {
        this.records.set(request.id, { id: request.id, status: "failed", result: null, error: error instanceof ServiceError ? error.message : "Codex returned an invalid proposal. No edits were applied." });
      }
    } finally {
      this.process = null;
      if (directory) await rm(directory, { recursive: true, force: true });
      this.activeId = null;
      while (this.records.size > 50) {
        const first = this.records.keys().next().value;
        if (first) this.records.delete(first); else break;
      }
    }
  }
}
