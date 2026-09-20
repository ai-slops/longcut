import { z } from "zod"

const Id = z.string().min(1)
const Frame = z.string().regex(/^\d+$/)

export const CodexStatusSchema = z.object({
  installed: z.boolean(),
  version: z.string().nullable(),
  state: z.enum(["logged_out", "login_pending", "ready", "busy", "error"]),
  auth_mode: z.enum(["none", "chatgpt", "unsupported"]),
  login: z
    .object({ url: z.string().url(), code: z.string().min(1), expires_at: z.string().min(1) })
    .nullable(),
  error: z.string().nullable(),
})

export const AiScopeSchema = z.discriminatedUnion("type", [
  z.object({ type: z.literal("source"), source_id: Id, start_frame: Frame, end_frame: Frame }),
  z.object({ type: z.literal("scenes"), scene_ids: z.array(Id).min(1).max(12) }),
])

export const AiRequestSchema = z.object({
  base_revision: Frame,
  instruction: z.string().min(1).max(8_000),
  consent: z.literal(true),
  scope: AiScopeSchema,
})

export const AiRunSchema = z.object({
  id: Id,
  project_id: Id,
  base_revision: Frame,
  instruction: z.string(),
  scope: AiScopeSchema,
  status: z.enum(["queued", "sampling", "generating", "ready", "error", "cancelled", "stale"]),
  created_at: z.string(),
  updated_at: z.string(),
  change_set_id: Id.nullable(),
  summary: z.string().nullable(),
  warnings: z.array(z.string()),
  sample_count: z.number().int().min(0).max(12),
  error: z.string().nullable(),
})

export const AiRunsSchema = z.array(AiRunSchema).max(50)

export type CodexStatus = z.infer<typeof CodexStatusSchema>
export type AiScope = z.infer<typeof AiScopeSchema>
export type AiRequest = z.infer<typeof AiRequestSchema>
export type AiRun = z.infer<typeof AiRunSchema>
