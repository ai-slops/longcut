import { messageItem } from "@slop-lab/part/context/index.js";
import { bearerAuth, createFetchTransport, noAuth } from "@slop-lab/part/model/index.js";
import type { Context, Contracts, Convenience, Model } from "@slop-lab/part/reference/index.js";
import { fail, ok, schema } from "@slop-lab/part/contracts/index.js";
import { createAgent } from "@slop-lab/part-plugins/agent/index.js";
import { createBasicDriver, createValidatingDriver } from "@slop-lab/part-plugins/agent/drivers/basic.js";
import { createChatDecoder } from "@slop-lab/part-plugins/providers/openai-compatible/decode.js";
import { digest } from "@slop-lab/part/contracts/index.js";
import { version as partVersion } from "@slop-lab/part/version.js";
import { version as pluginsVersion } from "@slop-lab/part-plugins/version.js";
import type { OpenAiConfig } from "./config.ts";
import { BoundedFetchTransport } from "./fetch-transport.ts";
import { JobRequestSchema, ProposalSchema, ServiceError, type JobRequest, type Proposal, type ProviderId } from "./types.ts";

export const ENGINE = "@slop-lab/part" as const;
export const ENGINE_VERSION = "0.1.0" as const;
if (partVersion !== "0.1.0" || pluginsVersion !== "0.1.0") {
  throw new ServiceError("RUNTIME_VERSION", 503, "Unsupported PART package version.");
}
export const PROFILE_REVISIONS = {
  codex: "longcut-codex-v1",
  "openai-compatible": "longcut-openai-compatible-v1",
} as const;

const capabilities: Contracts.CapabilitySet = { revision: "longcut-tool-free-v1", entries: [] };
const proposalOutput = schema<Proposal>("longcut/proposal", (value): value is Proposal => ProposalSchema.safeParse(value).success);

function nativeRequest(invocation: Model.Invocation): Contracts.Result<JobRequest> {
  for (const fragment of invocation.context.fragments) {
    for (const part of fragment.content) {
      if (part.kind === "native" && part.native.provider === "longcut/job") {
        const parsed = JobRequestSchema.safeParse(part.native.payload);
        return parsed.success ? ok(parsed.data) : fail("longcut.input", "Invalid native job payload");
      }
    }
  }
  return fail("longcut.input", "Missing native job payload");
}

function validatedDriver(profileId: string) {
  return createValidatingDriver(
    createBasicDriver({ profileId, tools: [], modelCapabilities: capabilities, maxTurns: 1 }),
    proposalOutput,
    async (response) => {
      if (response.finish.normalized !== "stop" || response.toolCalls.length !== 0) {
        return fail("longcut.incomplete", "Model did not return a complete tool-free proposal");
      }
      if (response.content.some((part) => part.kind !== "text")) {
        return fail("longcut.content", "Model returned unsupported content");
      }
      const text = response.content.flatMap((part) => part.kind === "text" ? [part.text] : []).join("");
      try {
        const parsed: unknown = JSON.parse(text);
        const proposal = ProposalSchema.safeParse(parsed);
        return proposal.success ? ok(proposal.data) : fail("longcut.proposal", "Model returned an invalid proposal");
      } catch {
        return fail("longcut.proposal", "Model returned invalid JSON");
      }
    },
  );
}

function compatibleDecoder(response: { readonly status?: number; readonly headers: Readonly<Record<string, string>> }) {
  return createChatDecoder(response);
}

export function createVisionAdapter(config: OpenAiConfig): Model.Adapter {
  return {
    id: "longcut/openai-vision",
    async encode(invocation, profile) {
      if (invocation.tools.length || invocation.continuation) return fail("longcut.profile", "Tools and continuations are disabled", "unsupported");
      const input = nativeRequest(invocation);
      if (!input.ok) return input;
      const content: Contracts.Json[] = [
        { type: "text", text: input.value.prompt },
        ...input.value.images.map((image) => ({ type: "image_url", image_url: { url: `data:image/jpeg;base64,${image}` } })),
      ];
      const responseFormat: Contracts.Json | undefined = config.responseFormat === "json_schema"
        ? { type: "json_schema", json_schema: { name: "ai_proposal", strict: true, schema: input.value.schema } }
        : config.responseFormat === "json_object" ? { type: "json_object" } : undefined;
      const body = new TextEncoder().encode(JSON.stringify({
        model: profile.model,
        messages: [{ role: "user", content }],
        ...(responseFormat ? { response_format: responseFormat } : {}),
        [config.tokenParameter]: config.maxOutputTokens,
        stream: true,
      }));
      return ok({ protocol: "openai.chat.completions", mediaType: "application/json", body, digest: await digest(body), metadata: { jobId: input.value.id }, inspectionBoundary: "outgoing-body" });
    },
    createDecoder: compatibleDecoder,
  };
}

export function createCodexAdapter(): Model.Adapter {
  return {
    id: "longcut/codex-broker",
    async encode(invocation) {
      if (invocation.tools.length || invocation.continuation) return fail("longcut.profile", "Tools and continuations are disabled", "unsupported");
      const input = nativeRequest(invocation);
      if (!input.ok) return input;
      const body = new TextEncoder().encode(JSON.stringify(input.value));
      return ok({ protocol: "longcut.codex", mediaType: "application/json", body, digest: await digest(body), metadata: {}, inspectionBoundary: "broker-input" });
    },
    createDecoder: compatibleDecoder,
  };
}

export async function partInput(request: JobRequest): Promise<readonly Context.SemanticItem[]> {
  return [await messageItem("user", [{ kind: "native", native: { provider: "longcut/job", formatVersion: "1", payload: request } }])];
}

type Agent = Convenience.Agent<readonly Context.SemanticItem[], Proposal>;
function unwrapAgent(result: Contracts.Result<Agent>): Agent {
  if (!result.ok) throw new ServiceError("RUNTIME_UNAVAILABLE", 503, "PART agent could not be initialized.");
  return result.value;
}

export function createVisionRuntime(config: OpenAiConfig): { readonly agent: Agent; readonly transport: BoundedFetchTransport } {
  const adapter = createVisionAdapter(config);
  const fetchTransport = createFetchTransport({ allowedOrigins: [config.origin] });
  const transport = new BoundedFetchTransport(fetchTransport);
  const profile: Model.Profile = {
    id: "longcut/openai-compatible", revision: PROFILE_REVISIONS["openai-compatible"], adapterId: adapter.id,
    transportId: transport.id, authStrategyId: "longcut/openai-key", model: config.model, endpoint: config.endpoint,
    defaults: {}, capabilities,
  };
  const agent = unwrapAgent(createAgent({ model: { profile, adapter, transport, auth: bearerAuth(() => config.key) }, tools: [], driver: validatedDriver(profile.id), limits: { maxModelInvocations: 1, maxToolDispatches: 0, maxExecutorAttempts: 1 } }));
  return { agent, transport };
}

export function visionAgent(config: OpenAiConfig): Agent {
  return createVisionRuntime(config).agent;
}

export function codexAgent(transport: Model.Transport): Agent {
  const adapter = createCodexAdapter();
  const profile: Model.Profile = {
    id: "longcut/codex", revision: PROFILE_REVISIONS.codex, adapterId: adapter.id,
    transportId: transport.id, authStrategyId: "longcut/codex-account", model: "codex-cli", defaults: {}, capabilities,
  };
  return unwrapAgent(createAgent({ model: { profile, adapter, transport, auth: noAuth }, tools: [], driver: validatedDriver(profile.id), limits: { maxModelInvocations: 1, maxToolDispatches: 0, maxExecutorAttempts: 1 } }));
}

export function profileRevision(provider: ProviderId): string {
  return PROFILE_REVISIONS[provider];
}
