import type { Options } from "ky"
import { client, request } from "./api"
import type { AsrJob, AsrLanguage, AsrRuntimeStatus } from "./asr-domain"
import { AsrJobSchema, AsrRuntimeStatusSchema } from "./asr-domain"

type AsrRoute = {
  readonly projectId: string
  readonly sourceId: string
  readonly jobId?: string
  readonly signal: AbortSignal
}

function jobPath(route: AsrRoute): string {
  const sourcePath = `projects/${route.projectId}/sources/${route.sourceId}/asr`
  return route.jobId ? `${sourcePath}/${route.jobId}` : sourcePath
}

function requestOptions(signal: AbortSignal): Options {
  return { signal, timeout: false }
}

export const asrApi = {
  status: (signal: AbortSignal): Promise<AsrRuntimeStatus> =>
    request(() => client.get("asr/status", requestOptions(signal)).json(), AsrRuntimeStatusSchema),
  start: (route: AsrRoute, language: AsrLanguage): Promise<AsrJob> =>
    request(
      () =>
        client
          .post(jobPath(route), {
            ...requestOptions(route.signal),
            json: { language },
          })
          .json(),
      AsrJobSchema,
    ),
  get: (route: AsrRoute): Promise<AsrJob> =>
    request(() => client.get(jobPath(route), requestOptions(route.signal)).json(), AsrJobSchema),
  cancel: (route: AsrRoute): Promise<AsrJob> =>
    request(
      () => client.post(`${jobPath(route)}/cancel`, requestOptions(route.signal)).json(),
      AsrJobSchema,
    ),
}
