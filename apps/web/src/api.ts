import ky, { HTTPError } from "ky"
import { z } from "zod"
import {
  type AiRequest,
  type AiRun,
  AiRunSchema,
  AiRunsSchema,
  type CodexStatus,
  CodexStatusSchema,
} from "./ai-domain"
import {
  type Command,
  type Project,
  ProjectSchema,
  type ProjectSummary,
  ProjectsSchema,
  type Render,
  RenderSchema,
} from "./domain"

const ApiErrorSchema = z.object({ error: z.object({ code: z.string(), message: z.string() }) })
const client = ky.create({
  prefixUrl: `${window.location.origin}/api/v1`,
  timeout: 30_000,
  retry: { limit: 1, methods: ["get"] },
})

export class ApiError extends Error {
  readonly code: string
  readonly status: number

  constructor(code: string, message: string, status: number) {
    super(message)
    this.name = "ApiError"
    this.code = code
    this.status = status
  }
}

async function parseError(error: HTTPError): Promise<ApiError> {
  let body: unknown
  try {
    body = JSON.parse(await error.response.text())
  } catch {
    body = null
  }
  const result = ApiErrorSchema.safeParse(body)
  return result.success
    ? new ApiError(result.data.error.code, result.data.error.message, error.response.status)
    : new ApiError("http_error", `Request failed (${error.response.status})`, error.response.status)
}

async function request<T>(operation: () => Promise<unknown>, schema: z.ZodType<T>): Promise<T> {
  try {
    return schema.parse(await operation())
  } catch (error) {
    if (error instanceof HTTPError) throw await parseError(error)
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
