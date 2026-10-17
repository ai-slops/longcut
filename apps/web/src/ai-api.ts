import {
  type AiRequest,
  type AiRun,
  AiRunSchema,
  AiRunsSchema,
  type CodexStatus,
  CodexStatusSchema,
  type ProviderStatus,
  ProviderStatusesSchema,
  type RunRuntimeAvailability,
  RunRuntimeAvailabilitySchema,
  type RuntimeAvailability,
  RuntimeAvailabilitySchema,
} from "./ai-domain"
import { client, request } from "./api"
import { ApiError } from "./api-error"

export const aiApi = {
  codexStatus: (): Promise<CodexStatus> =>
    request(() => client.get("codex/status").json(), CodexStatusSchema),
  codexLogin: (): Promise<CodexStatus> =>
    request(() => client.post("codex/login").json(), CodexStatusSchema),
  cancelCodexLogin: (): Promise<CodexStatus> =>
    request(() => client.post("codex/login/cancel").json(), CodexStatusSchema),
  codexLogout: (): Promise<CodexStatus> =>
    request(() => client.post("codex/logout").json(), CodexStatusSchema),
  providers: (): Promise<readonly ProviderStatus[]> =>
    request(() => client.get("ai/providers").json(), ProviderStatusesSchema).then(
      ({ providers }) => providers,
    ),
  runtime: (signal: AbortSignal): Promise<RuntimeAvailability> =>
    request(() => client.get("ai/runtime", { signal }).json(), RuntimeAvailabilitySchema),
  listRuns: (projectId: string): Promise<readonly AiRun[]> =>
    request(() => client.get(`projects/${projectId}/ai/runs`).json(), AiRunsSchema),
  runRuntime: async (
    projectId: string,
    runId: string,
    signal: AbortSignal,
  ): Promise<RunRuntimeAvailability> => {
    const response = await request(
      () => client.get(`projects/${projectId}/ai/runs/${runId}/runtime`, { signal }).json(),
      RunRuntimeAvailabilitySchema,
    )
    if (response.run_id !== runId) {
      throw new ApiError("response_identity", "Runtime response identity mismatch.", 502)
    }
    return response
  },
  createRun: (projectId: string, input: AiRequest): Promise<AiRun> =>
    request(
      () => client.post(`projects/${projectId}/ai/runs`, { json: input }).json(),
      AiRunSchema,
    ),
  cancelRun: (projectId: string, runId: string): Promise<AiRun> =>
    request(() => client.post(`projects/${projectId}/ai/runs/${runId}/cancel`).json(), AiRunSchema),
}
