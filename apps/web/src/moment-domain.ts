import { z } from "zod"

const Id = z.string().min(1)
const Frame = z.string().regex(/^\d+$/)

export const MomentEvidenceSelectionSchema = z
  .object({
    transcript_id: Id,
    cue_ids: z.array(Id),
    word_ids: z.array(Id),
  })
  .strict()

export const MomentEvidenceSchema = MomentEvidenceSelectionSchema.extend({
  quote: z.string(),
  origin: z.enum(["subtitle_import", "local_asr"]),
  start_frame: Frame,
  end_frame: Frame,
  timing_basis: z.enum(["cues", "words"]),
}).strict()

export const NarrativeRoleSchema = z.enum([
  "context",
  "setup",
  "goal",
  "attempt",
  "failure",
  "insight",
  "payoff",
  "other",
])

export const MomentSchema = z
  .object({
    id: Id,
    version: Frame,
    source_id: Id,
    start_frame: Frame,
    end_frame: Frame,
    label: z.string(),
    note: z.string(),
    narrative_role: NarrativeRoleSchema,
    origin: z.literal("manual"),
    review_status: z.enum(["draft", "reviewed", "rejected"]),
    archived: z.boolean(),
    evidence: z.array(MomentEvidenceSchema).default([]),
    provenance: z.object({
      created_command_id: Id,
      updated_command_id: Id,
      created_revision: Frame,
      updated_revision: Frame,
      created_at: z.string(),
      updated_at: z.string(),
    }),
  })
  .strict()

export const MomentVersionResponseSchema = z
  .object({
    project_id: Id,
    moment_id: Id,
    version: Frame,
    snapshot: MomentSchema,
  })
  .strict()

export const SceneMomentSchema = z.object({ moment_id: Id, version: Frame }).strict()

export type MomentEvidenceSelection = z.infer<typeof MomentEvidenceSelectionSchema>
export type MomentEvidence = z.infer<typeof MomentEvidenceSchema>
export type Moment = z.infer<typeof MomentSchema>
