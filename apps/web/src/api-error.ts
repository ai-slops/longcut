import type { HTTPError } from "ky"
import { z } from "zod"

const ApiErrorSchema = z.object({ error: z.object({ code: z.string(), message: z.string() }) })

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

export async function parseApiError(error: HTTPError): Promise<ApiError> {
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
