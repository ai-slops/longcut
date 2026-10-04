import { z } from "zod"

const Id = z.string().min(1)

export const SessionAssetSchema = z.object({
  asset_id: Id,
  uri: z.string(),
  availability: z.enum(["online", "offline"]),
})

export const SessionSourceSchema = z.object({
  source_id: Id,
  display_name: z.string(),
  semantic_role: z.string(),
  kind: z.enum(["video", "audio", "av"]),
  editor_source_id: Id.nullable(),
  reason: z.string().nullable(),
})

export const SessionWarningSchema = z.object({
  code: z.string(),
  path: z.string(),
  message: z.string(),
})

export const SourceSessionSummarySchema = z.object({
  id: Id,
  session_id: Id,
  title: z.string(),
  duration_ns: z.string().regex(/^\d+$/),
  conformance: z.enum(["conformant", "degraded", "rejected"]),
  imported_at: z.string(),
  assets: z.array(SessionAssetSchema),
  sources: z.array(SessionSourceSchema),
  warnings: z.array(SessionWarningSchema),
  verification_url: z.string().nullable(),
})

export const SessionImportJobSchema = z.object({
  id: Id,
  project_id: Id,
  status: z.enum(["validating", "prepared", "publishing", "confirmed", "failed", "cancelled"]),
  created_at: z.string(),
  error: z.string().nullable(),
  session: SourceSessionSummarySchema.nullable(),
})

export const SessionImportJobsSchema = z.array(SessionImportJobSchema)

export type SourceSessionSummary = z.infer<typeof SourceSessionSummarySchema>
export type SessionImportJob = z.infer<typeof SessionImportJobSchema>
