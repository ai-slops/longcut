import type { Context, Convenience, Runtime } from "@slop-lab/part/reference/index.js";
import type { Auth } from "./auth.ts";
import type { CodexTransport } from "./codex-transport.ts";
import type { TransportCleanup } from "./fetch-transport.ts";
import { ENGINE, ENGINE_VERSION, partInput, profileRevision } from "./model.ts";
import { ServiceError, type JobRequest, type Proposal, type ProviderId, type RuntimeJob, type RuntimePhase } from "./types.ts";

type Agent = Convenience.Agent<readonly Context.SemanticItem[], Proposal>;
type Active = {
  readonly id: string;
  readonly provider: ProviderId;
  readonly handle: Runtime.ExecutionHandle<Proposal>;
};
type Cancellation = { readonly id: string; readonly settled: Promise<RuntimeJob> };
type TerminalState = {
  readonly status: Exclude<RuntimeJob["status"], "running">;
  readonly result: Proposal | null;
  readonly error: RuntimeJob["error"];
};
type Services = {
  readonly auth: Auth;
  readonly codex: Agent;
  readonly codexTransport: CodexTransport;
  readonly openAi: Agent | null;
  readonly openAiTransport: TransportCleanup | null;
  readonly scheduleTimeout?: (callback: () => void, milliseconds: number) => NodeJS.Timeout;
};

export class Jobs {
  private readonly records = new Map<string, RuntimeJob>();
  private active: Active | null = null;
  private completion: Promise<void> | null = null;
  private cancellation: Cancellation | null = null;
  private pendingStart: Promise<RuntimeJob> | null = null;
  private stopping = false;
  private readonly services: Services;
  constructor(services: Services) { this.services = services; }

  busy(): boolean { return this.active !== null || this.pendingStart !== null; }
  get(id: string): RuntimeJob {
    const value = this.records.get(id);
    if (!value) throw new ServiceError("NOT_FOUND", 404, "AI job not found; it may have been interrupted by a restart.");
    return value;
  }
  async start(request: JobRequest): Promise<RuntimeJob> {
    if (this.stopping) throw new ServiceError("SHUTTING_DOWN", 503, "Agent runtime is shutting down.");
    if (this.records.has(request.id)) throw new ServiceError("DUPLICATE_JOB", 409, "Job ID already exists.");
    if (this.busy() || this.services.auth.pending()) throw new ServiceError("RUNTIME_BUSY", 409, "Another account operation or AI run is active.");
    this.pendingStart = this.admit(request);
    try { return await this.pendingStart; } finally { this.pendingStart = null; }
  }
  private async admit(request: JobRequest): Promise<RuntimeJob> {
    if (request.provider === "codex") {
      const status = await this.services.auth.status();
      if (!status.installed) throw new ServiceError("CODEX_UNAVAILABLE", 503, "Codex safety verification failed; OpenAI-compatible generation remains available.");
      if (status.auth_mode !== "chatgpt") throw new ServiceError("AUTH_REQUIRED", 401, "Connect your ChatGPT account before generating an edit.");
    }
    if (this.stopping) throw new ServiceError("SHUTTING_DOWN", 503, "Agent runtime is shutting down.");
    const agent = request.provider === "codex" ? this.services.codex : this.services.openAi;
    if (!agent) throw new ServiceError("PROVIDER_UNAVAILABLE", 409, "OpenAI-compatible provider is not configured.");
    const started = await agent.start(await partInput(request));
    if (!started.ok) throw new ServiceError("RUNTIME_UNAVAILABLE", 503, "PART did not admit the generation.");
    const job: RuntimeJob = {
      id: request.id, status: "running", result: null, error: null,
      runtime: { engine: ENGINE, engine_version: ENGINE_VERSION, execution_id: started.value.ref.id, profile_revision: profileRevision(request.provider) },
      progress: { sequence: 0, phase: "admitted" },
    };
    this.records.set(job.id, job);
    this.active = { id: job.id, provider: request.provider, handle: started.value };
    this.completion = this.execute(this.active);
    return job;
  }
  async cancel(id: string): Promise<RuntimeJob> {
    const value = this.get(id);
    const active = this.active?.id === id ? this.active : null;
    if (!active) return value;
    if (this.cancellation?.id === id) return this.cancellation.settled;
    const settled = this.cancelActive(active);
    this.cancellation = { id, settled };
    return settled;
  }
  async waitForIdle(): Promise<void> { await this.completion; }
  async shutdown(): Promise<void> {
    this.stopping = true;
    try { await this.pendingStart; } catch (error) { if (!(error instanceof ServiceError)) throw error; }
    if (this.active) await this.cancel(this.active.id);
    await this.waitForIdle();
    await Promise.all([this.services.codexTransport.shutdown(), this.services.openAiTransport?.shutdown(), this.services.codex.dispose(), this.services.openAi?.dispose()]);
  }
  private progress(id: string, phase: RuntimePhase): void {
    const job = this.get(id);
    if (job.status === "running" || phase === "terminal") this.records.set(id, { ...job, progress: { sequence: job.progress.sequence + 1, phase } });
  }
  private async execute(active: Active): Promise<void> {
    let timedOut = false;
    let terminal: TerminalState = { status: "failed", result: null, error: { code: "PROVIDER_FAILED", message: "The provider did not return a valid proposal. No edits were applied." } };
    const timeout = (this.services.scheduleTimeout ?? setTimeout)(() => {
      timedOut = true;
      void active.handle.requestCancel("generation-timeout");
      void this.cleanup(active);
    }, 600_000);
    timeout.unref();
    try {
      this.progress(active.id, "provider");
      const outcome = await active.handle.done;
      const current = this.get(active.id);
      if (current.status === "cancelled") {
        terminal = { status: "cancelled", result: null, error: null };
      } else if (timedOut) {
        this.progress(active.id, "validating");
        terminal = { status: "failed", result: null, error: { code: "TIMEOUT", message: "The provider did not complete within ten minutes. No edits were applied." } };
      } else if (outcome.phase === "completed") {
        this.progress(active.id, "validating");
        terminal = { status: "succeeded", result: outcome.result, error: null };
      } else if (outcome.phase === "cancelled") {
        terminal = { status: "cancelled", result: null, error: null };
      } else {
        this.progress(active.id, "validating");
        const tool = outcome.error.code.includes("tool-attempt");
        terminal = { status: "failed", result: null, error: { code: tool ? "TOOL_ATTEMPT" : "PROVIDER_FAILED", message: tool ? "The provider attempted a tool; the proposal was discarded." : "The provider did not return a valid proposal. No edits were applied." } };
      }
    } finally {
      clearTimeout(timeout);
      const job = this.get(active.id);
      this.records.set(active.id, { ...job, progress: { sequence: job.progress.sequence + 1, phase: "cleaning_up" } });
      await this.cleanup(active);
      const cleaned = this.get(active.id);
      const published: TerminalState = cleaned.status === "cancelled" ? { status: "cancelled", result: null, error: null } : terminal;
      this.records.set(active.id, { ...cleaned, ...published, progress: { sequence: cleaned.progress.sequence + 1, phase: "terminal" } });
      this.active = null;
      while (this.records.size > 50) {
        const first = this.records.keys().next().value;
        if (typeof first === "string") this.records.delete(first);
      }
    }
  }

  private async cancelActive(active: Active): Promise<RuntimeJob> {
    const value = this.get(active.id);
    if (value.status === "running") this.records.set(active.id, { ...value, status: "cancelled", result: null, error: null, progress: { sequence: value.progress.sequence + 1, phase: "cleaning_up" } });
    await active.handle.requestCancel("user-request");
    await this.cleanup(active);
    await active.handle.done;
    await this.completion;
    return this.get(active.id);
  }

  private async cleanup(active: Active): Promise<void> {
    const transport = active.provider === "codex" ? this.services.codexTransport : this.services.openAiTransport;
    await transport?.cancel(active.id);
  }
}
