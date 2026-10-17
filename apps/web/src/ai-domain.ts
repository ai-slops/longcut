import { z } from "zod"

const Id = z.string().min(1)
const Frame = z.string().regex(/^\d+$/)

export const AiProviderSchema = z.enum(["codex", "openai-compatible"])

export const ProviderStatusSchema = z.object({
  id: AiProviderSchema,
  name: z.string().min(1),
  configured: z.boolean(),
  model: z.string().nullable(),
  endpoint: z.string().nullable(),
  detail: z.string().nullable(),
})

export const ProviderStatusesSchema = z.object({
  providers: z.array(ProviderStatusSchema),
})

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
  provider: AiProviderSchema.default("codex"),
  base_revision: Frame,
  instruction: z.string().min(1).max(8_000),
  consent: z.literal(true),
  scope: AiScopeSchema,
})

export const AiRunSchema = z.object({
  id: Id,
  project_id: Id,
  provider: AiProviderSchema.default("codex"),
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

export const RuntimeAvailabilitySchema = z.object({
  available: z.boolean(),
  engine: z.literal("@slop-lab/part"),
  version: z.string().nullable(),
  error: z.string().nullable(),
})

export const RuntimeMetadataSchema = z.object({
  engine: z.literal("@slop-lab/part"),
  engine_version: z.string().min(1),
  execution_id: Id,
  profile_revision: z.string().min(1),
})

export const RuntimeProgressSchema = z.object({
  sequence: z.number().int().nonnegative(),
  phase: z.enum(["admitted", "provider", "validating", "cleaning_up", "terminal"]),
})

export const RunRuntimeAvailabilitySchema = z.object({
  run_id: Id,
  available: z.boolean(),
  runtime: RuntimeMetadataSchema.nullable(),
  progress: RuntimeProgressSchema.nullable(),
})

export type CodexStatus = z.infer<typeof CodexStatusSchema>
export type AiProvider = z.infer<typeof AiProviderSchema>
export type ProviderStatus = z.infer<typeof ProviderStatusSchema>
export type AiScope = z.infer<typeof AiScopeSchema>
export type AiRequest = z.infer<typeof AiRequestSchema>
export type AiRun = z.infer<typeof AiRunSchema>
export type RuntimeAvailability = z.infer<typeof RuntimeAvailabilitySchema>
export type RunRuntimeAvailability = z.infer<typeof RunRuntimeAvailabilitySchema>
