import ky, { HTTPError } from "ky"
import type { z } from "zod"
import {
  type AiRequest,
  type AiRun,
  AiRunSchema,
  AiRunsSchema,
  type CodexStatus,
  CodexStatusSchema,
  type ProviderStatus,
  ProviderStatusesSchema,
} from "./ai-domain"
import { ApiError, parseApiError } from "./api-error"
import {
  type Command,
  type Moment,
  MomentVersionResponseSchema,
  type Project,
  ProjectSchema,
  type ProjectSummary,
  ProjectsSchema,
  type Render,
  RenderSchema,
  type Story,
  StoryVersionResponseSchema,
} from "./domain"
import {
  type MomentVersionFetch,
  momentVersionIdentity,
  versionResponseMatchesIdentity,
} from "./moment-version-request"
import {
  type SessionImportJob,
  SessionImportJobSchema,
  SessionImportJobsSchema,
} from "./session-domain"
import type { PackageFile } from "./session-files"
import {
  type StoryVersionIdentity,
  storyVersionResponseMatchesIdentity,
} from "./story-version-request"
import { type SyncDiagnostics, SyncDiagnosticsSchema } from "./sync-diagnostics-domain"
import {
  OptionalTranscriptPageSchema,
  type TranscriptPage,
  TranscriptPageSchema,
} from "./transcript-domain"

export { ApiError } from "./api-error"

export const client = ky.create({
  prefixUrl: `${window.location.origin}/api/v1`,
  timeout: 30_000,
  retry: { limit: 1, methods: ["get"] },
})

export async function request<T>(
  operation: () => Promise<unknown>,
  schema: z.ZodType<T>,
): Promise<T> {
  try {
    return schema.parse(await operation())
  } catch (error) {
    if (error instanceof HTTPError) throw await parseApiError(error)
    throw error
  }
}

export const api = {
  listProjects: (): Promise<readonly ProjectSummary[]> =>
    request(() => client.get("projects").json(), ProjectsSchema),
  createProject: (title: string, frameRate: 30 | 60): Promise<Project> =>
    request(
      () => client.post("projects", { json: { title, frame_rate: frameRate } }).json(),
      ProjectSchema,
    ),
  getProject: (id: string): Promise<Project> =>
    request(() => client.get(`projects/${id}`).json(), ProjectSchema),
  command: (project: Project, command: Command): Promise<Project> =>
    request(
      () =>
        client
          .post(`projects/${project.id}/commands`, {
            json: { command_id: crypto.randomUUID(), base_revision: project.revision, command },
          })
          .json(),
      ProjectSchema,
    ),
  getMomentVersion: async ({
    projectId,
    momentId,
    version,
    signal,
  }: MomentVersionFetch): Promise<Moment> => {
    const response = await request(
      () =>
        client
          .get(`projects/${projectId}/moments/${momentId}/versions/${version}`, {
            signal,
          })
          .json(),
      MomentVersionResponseSchema,
    )
    if (
      !versionResponseMatchesIdentity(momentVersionIdentity(projectId, momentId, version), response)
    ) {
      throw new ApiError("response_identity", "Moment version response identity mismatch.", 502)
    }
    return response.snapshot
  },
  getStoryVersion: async (identity: StoryVersionIdentity, signal: AbortSignal): Promise<Story> => {
    const response = await request(
      () =>
        client
          .get(`projects/${identity.projectId}/story/versions/${identity.version}`, { signal })
          .json(),
      StoryVersionResponseSchema,
    )
    if (!storyVersionResponseMatchesIdentity(identity, response)) {
      throw new ApiError("response_identity", "Story version response identity mismatch.", 502)
    }
    return response.snapshot
  },
  importMedia: (
    project: Project,
    file: File,
    role: "program" | "primary" | "camera" | "audio",
  ): Promise<Project> => {
    const body = new FormData()
    body.set("file", file)
    return request(
      () =>
        client
          .post(`projects/${project.id}/imports`, { searchParams: { role }, body, timeout: false })
          .json(),
      ProjectSchema,
    )
  },
  createSessionImport: (
    projectId: string,
    files: readonly PackageFile[],
  ): Promise<SessionImportJob> => {
    const body = new FormData()
    for (const { file, path } of files) body.append("file", file, path)
    return request(
      () => client.post(`projects/${projectId}/session-imports`, { body, timeout: false }).json(),
      SessionImportJobSchema,
    )
  },
  listSessionImports: (projectId: string): Promise<readonly SessionImportJob[]> =>
    request(
      () => client.get(`projects/${projectId}/session-imports`).json(),
      SessionImportJobsSchema,
    ),
  getSessionImport: (projectId: string, importId: string): Promise<SessionImportJob> =>
    request(
      () => client.get(`projects/${projectId}/session-imports/${importId}`).json(),
      SessionImportJobSchema,
    ),
  confirmSessionImport: (
    project: Project,
    importId: string,
    allowDegraded: boolean,
  ): Promise<Project> =>
    request(
      () =>
        client
          .post(`projects/${project.id}/session-imports/${importId}/confirm`, {
            json: { base_revision: project.revision, allow_degraded: allowDegraded },
            timeout: false,
          })
          .json(),
      ProjectSchema,
    ),
  cancelSessionImport: (projectId: string, importId: string): Promise<SessionImportJob> =>
    request(
      () => client.post(`projects/${projectId}/session-imports/${importId}/cancel`).json(),
      SessionImportJobSchema,
    ),
  getSyncDiagnostics: (projectId: string, sessionId: string): Promise<SyncDiagnostics> =>
    request(
      () =>
        client.get(`projects/${projectId}/source-sessions/${sessionId}/sync-diagnostics`).json(),
      SyncDiagnosticsSchema,
    ),
  getTranscript: (
    projectId: string,
    sourceId: string,
    options: { readonly query: string; readonly cursor?: string; readonly signal: AbortSignal },
  ): Promise<TranscriptPage | null> =>
    request(
      () =>
        client
          .get(`projects/${projectId}/sources/${sourceId}/transcript`, {
            searchParams: {
              query: options.query,
              ...(options.cursor ? { cursor: options.cursor } : {}),
            },
            signal: options.signal,
          })
          .json(),
      OptionalTranscriptPageSchema,
    ),
  importTranscript: (
    projectId: string,
    sourceId: string,
    input: { readonly file: File; readonly signal: AbortSignal },
  ): Promise<TranscriptPage> =>
    request(
      async () =>
        client
          .post(`projects/${projectId}/sources/${sourceId}/transcripts`, {
            json: { filename: input.file.name, text: await input.file.text() },
            signal: input.signal,
          })
          .json(),
      TranscriptPageSchema,
    ),
  createDemo: (project: Project): Promise<Project> =>
    request(
      () => client.post(`projects/${project.id}/demo`, { timeout: false }).json(),
      ProjectSchema,
    ),
  render: (project: Project, kind: "preview" | "final"): Promise<Render> =>
    request(
      () =>
        client
          .post(`projects/${project.id}/renders`, {
            json: { base_revision: project.revision, kind },
            timeout: false,
          })
          .json(),
      RenderSchema,
    ),
  codexStatus: (): Promise<CodexStatus> =>
    request(() => client.get("codex/status").json(), CodexStatusSchema),
  codexLogin: (): Promise<CodexStatus> =>
    request(() => client.post("codex/login").json(), CodexStatusSchema),
  cancelCodexLogin: (): Promise<CodexStatus> =>
    request(() => client.post("codex/login/cancel").json(), CodexStatusSchema),
  codexLogout: (): Promise<CodexStatus> =>
    request(() => client.post("codex/logout").json(), CodexStatusSchema),
  aiProviders: (): Promise<readonly ProviderStatus[]> =>
    request(() => client.get("ai/providers").json(), ProviderStatusesSchema).then(
      ({ providers }) => providers,
    ),
  listAiRuns: (projectId: string): Promise<readonly AiRun[]> =>
    request(() => client.get(`projects/${projectId}/ai/runs`).json(), AiRunsSchema),
  createAiRun: (projectId: string, input: AiRequest): Promise<AiRun> =>
    request(
      () => client.post(`projects/${projectId}/ai/runs`, { json: input }).json(),
      AiRunSchema,
    ),
  cancelAiRun: (projectId: string, runId: string): Promise<AiRun> =>
    request(() => client.post(`projects/${projectId}/ai/runs/${runId}/cancel`).json(), AiRunSchema),
}
