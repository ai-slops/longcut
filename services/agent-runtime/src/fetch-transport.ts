import { fail, isJsonObject, ok } from "@slop-lab/part/contracts/index.js";
import type { Model } from "@slop-lab/part/reference/index.js";

export interface TransportCleanup {
  cancel(id: string): Promise<void>;
  shutdown(): Promise<void>;
}
type Attempt = {
  readonly cancellation: AbortController;
  readonly settled: Promise<void>;
  readonly settle: () => void;
};

export class BoundedFetchTransport implements Model.Transport, TransportCleanup {
  readonly id = "longcut/bounded-fetch";
  private readonly transport: Model.Transport;
  private readonly active = new Map<string, Attempt>();
  private stopping = false;
  constructor(transport: Model.Transport) { this.transport = transport; }

  async cancel(id: string): Promise<void> {
    const attempt = this.active.get(id);
    attempt?.cancellation.abort();
    await attempt?.settled;
  }

  async shutdown(): Promise<void> {
    this.stopping = true;
    const attempts = [...this.active.values()];
    for (const attempt of attempts) attempt.cancellation.abort();
    await Promise.all(attempts.map((attempt) => attempt.settled));
  }

  async send(request: Model.EncodedRequest, profile: Model.Profile, auth: Model.CredentialBinding, context: Parameters<Model.Transport["send"]>[3]) {
    const jobId = isJsonObject(request.metadata) && typeof request.metadata["jobId"] === "string" ? request.metadata["jobId"] : null;
    if (!jobId) return fail("longcut.job-id", "Provider request is missing its job ID");
    if (this.stopping) return fail("longcut.stopping", "Provider transport is stopping", "cancelled", "not-attempted");
    if (this.active.has(jobId)) return fail("longcut.active", "Provider transport attempt already exists", "conflict", "not-attempted");
    const completion = Promise.withResolvers<void>();
    const attempt: Attempt = { cancellation: new AbortController(), settled: completion.promise, settle: completion.resolve };
    this.active.set(jobId, attempt);
    const cancel = () => attempt.cancellation.abort();
    context.signal.addEventListener("abort", cancel, { once: true });
    if (context.signal.aborted) cancel();
    try {
      return await this.execute(request, profile, auth, { ...context, signal: attempt.cancellation.signal });
    } finally {
      context.signal.removeEventListener("abort", cancel);
      attempt.settle();
      this.active.delete(jobId);
    }
  }

  private async execute(request: Model.EncodedRequest, profile: Model.Profile, auth: Model.CredentialBinding, context: Parameters<Model.Transport["send"]>[3]) {
    const sent = await this.transport.send(request, profile, auth, context);
    if (!sent.ok) return sent;
    const chunks: Uint8Array[] = [];
    let bytes = 0;
    try {
      for await (const chunk of sent.value.body) {
        bytes += chunk.byteLength;
        if (bytes > 262_144) return fail("longcut.output-limit", "Provider response exceeded the output limit");
        chunks.push(Uint8Array.from(chunk));
      }
    } catch {
      return fail("longcut.transport", "Provider response stream failed", context.signal.aborted ? "cancelled" : "unavailable");
    }
    return ok({ ...sent.value, body: (async function* () { yield* chunks; })() });
  }
}
