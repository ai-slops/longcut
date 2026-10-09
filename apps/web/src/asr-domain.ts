import { z } from "zod"

export const AsrLanguageSchema = z.enum(["ko", "en", "auto"])

const AsrProgressSchema = z
  .object({
    completed_chunks: z.number().int().nonnegative(),
    total_chunks: z.number().int().nonnegative(),
  })
  .refine(
    ({ completed_chunks, total_chunks }) => completed_chunks <= total_chunks,
    "completed ASR chunks cannot exceed total chunks",
  )

export const AsrRuntimeStatusSchema = z.object({
  configured: z.boolean(),
  available: z.boolean(),
  cli_available: z.boolean(),
  model_available: z.boolean(),
  ffmpeg_available: z.boolean(),
  missing: z.array(z.enum(["LONGCUT_WHISPER_CLI", "LONGCUT_WHISPER_MODEL"])),
  engine: z.literal("whisper.cpp"),
  model: z.string().nullable(),
  default_language: z.literal("ko"),
  supported_languages: z.tuple([z.literal("ko"), z.literal("en"), z.literal("auto")]),
  chunk_seconds: z.number().int().min(30).max(3600),
  max_concurrency: z.number().int().min(1).max(4),
})

export const AsrJobSchema = z.object({
  id: z.string().min(1),
  project_id: z.string().min(1),
  source_id: z.string().min(1),
  status: z.enum(["queued", "running", "ready", "failed", "cancelled"]),
  progress: AsrProgressSchema,
  language: AsrLanguageSchema,
  engine: z.literal("whisper.cpp"),
  model: z.string(),
  created_at: z.string(),
  updated_at: z.string(),
  transcript_id: z.string().nullable(),
  error: z.string().nullable(),
})

export type AsrLanguage = z.infer<typeof AsrLanguageSchema>
export type AsrRuntimeStatus = z.infer<typeof AsrRuntimeStatusSchema>
export type AsrJob = z.infer<typeof AsrJobSchema>

export function asrJobStorageKey(projectId: string, sourceId: string): string {
  return `longcut:asr:${encodeURIComponent(projectId)}:${encodeURIComponent(sourceId)}`
}

export function asrJobBelongsToSource(job: AsrJob, projectId: string, sourceId: string): boolean {
  return job.project_id === projectId && job.source_id === sourceId
}

export function isActiveAsrJob(job: AsrJob): boolean {
  return job.status === "queued" || job.status === "running"
}

export function asrRequestMatches(
  currentRequestVersion: number,
  responseRequestVersion: number,
): boolean {
  return currentRequestVersion === responseRequestVersion
}

export function canRetryAsrPoll(job: AsrJob, pollFailed: boolean): boolean {
  return pollFailed && isActiveAsrJob(job)
}

export function canPollAsrJob(job: AsrJob, pollFailed: boolean, submitting: boolean): boolean {
  return isActiveAsrJob(job) && !pollFailed && !submitting
}
