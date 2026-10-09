import { z } from "zod"
import { MomentSchema, SceneMomentSchema } from "./moment-domain.ts"
import { SourceSessionSummarySchema } from "./session-domain.ts"

export type { Command } from "./command.ts"
export {
  type Moment,
  type MomentEvidence,
  MomentEvidenceSchema,
  type MomentEvidenceSelection,
  MomentEvidenceSelectionSchema,
  MomentSchema,
  MomentVersionResponseSchema,
  NarrativeRoleSchema,
  SceneMomentSchema,
} from "./moment-domain.ts"

const Id = z.string().min(1)
const Frame = z.string().regex(/^\d+$/)
const Point = z.object({ x: z.number(), y: z.number() })

export const CameraSchema = z.object({
  source_id: Id,
  in_frame: Frame,
  x: z.number(),
  y: z.number(),
  width: z.number().positive(),
  height: z.number().positive(),
  opacity: z.number().min(0).max(1),
})

export const SourceSchema = z.object({
  id: Id,
  name: z.string(),
  role: z.enum(["program", "primary", "camera", "audio"]),
  duration_frames: Frame,
  width: z.number().int().nonnegative(),
  height: z.number().int().nonnegative(),
  has_audio: z.boolean(),
  media_url: z.string(),
  proxy_url: z.string().nullable(),
  thumbnail_url: z.string().nullable(),
  waveform: z.array(z.number()),
  status: z.enum(["processing", "ready", "error"]),
  error: z.string().nullable(),
})

export const SceneSchema = z.object({
  id: Id,
  title: z.string(),
  purpose: z.string(),
  source_id: Id,
  in_frame: Frame,
  out_frame: Frame,
  camera: CameraSchema.nullable(),
  caption: z.string(),
  locked: z.boolean(),
  manual: z.boolean(),
  moment: SceneMomentSchema.nullable().default(null),
})

export const FeedbackSchema = z.object({
  id: Id,
  body: z.string(),
  scene_id: Id,
  start_frame: Frame,
  end_frame: Frame,
  revision: Frame,
  render_id: Id.nullable(),
  pin: Point.nullable(),
  resolved: z.boolean(),
})

const OperationBase = z.object({
  id: Id,
  scene_id: Id,
  rationale: z.string(),
})
export const OperationSchema = z.discriminatedUnion("type", [
  OperationBase.extend({
    type: z.literal("set_camera"),
    camera: CameraSchema.nullable(),
    in_frame: Frame.nullish(),
    out_frame: Frame.nullish(),
    source_id: Id.nullish(),
    title: z.string().nullish(),
    purpose: z.string().nullish(),
    caption: z.string().nullish(),
  }),
  OperationBase.extend({
    type: z.literal("trim_scene"),
    camera: CameraSchema.nullish(),
    in_frame: Frame,
    out_frame: Frame,
    source_id: Id.nullish(),
    title: z.string().nullish(),
    purpose: z.string().nullish(),
    caption: z.string().nullish(),
  }),
  OperationBase.extend({
    type: z.literal("insert_scene"),
    source_id: Id,
    in_frame: Frame,
    out_frame: Frame,
    title: z.string(),
    purpose: z.string().nullish(),
    caption: z.string().nullish(),
    camera: CameraSchema.nullish(),
  }),
  OperationBase.extend({
    type: z.literal("update_scene"),
    source_id: Id.nullish(),
    in_frame: Frame.nullish(),
    out_frame: Frame.nullish(),
    title: z.string(),
    purpose: z.string(),
    caption: z.string(),
    camera: CameraSchema.nullish(),
  }),
  OperationBase.extend({
    type: z.literal("delete_scene"),
    source_id: Id.nullish(),
    in_frame: Frame.nullish(),
    out_frame: Frame.nullish(),
    title: z.string().nullish(),
    purpose: z.string().nullish(),
    caption: z.string().nullish(),
    camera: CameraSchema.nullish(),
  }),
])

export const ChangeSetSchema = z.object({
  id: Id,
  title: z.string(),
  intent: z.string(),
  base_revision: Frame,
  operations: z.array(OperationSchema),
  status: z.enum(["ready", "partially-applied", "applied", "rejected"]),
  applied_operation_ids: z.array(Id),
  evidence: z.array(z.string()),
})

export const RenderSchema = z.object({
  id: Id,
  revision: Frame,
  kind: z.enum(["preview", "final"]),
  status: z.enum(["queued", "running", "ready", "error"]),
  progress: z.number().min(0).max(1),
  media_url: z.string().nullable(),
  srt_url: z.string().nullable(),
  vtt_url: z.string().nullable(),
  lineage_url: z.string().nullable(),
  error: z.string().nullable(),
})

export const ProjectSchema = z.object({
  id: Id,
  title: z.string(),
  revision: Frame,
  frame_rate: z.union([z.literal(30), z.literal(60)]),
  updated_at: z.string(),
  brief: z.string(),
  sources: z.array(SourceSchema),
  source_sessions: z.array(SourceSessionSummarySchema).default([]),
  scenes: z.array(SceneSchema),
  moments: z.array(MomentSchema).default([]),
  feedback: z.array(FeedbackSchema),
  change_sets: z.array(ChangeSetSchema),
  renders: z.array(RenderSchema),
  can_undo: z.boolean(),
  can_redo: z.boolean(),
})

export const ProjectSummarySchema = z.object({
  id: Id,
  title: z.string(),
  revision: Frame,
  updated_at: z.string(),
})

export const ProjectsSchema = z.array(ProjectSummarySchema)
export const HealthSchema = z.record(z.string(), z.unknown())

export type Camera = z.infer<typeof CameraSchema>
export type Source = z.infer<typeof SourceSchema>
export type Scene = z.infer<typeof SceneSchema>
export type Feedback = z.infer<typeof FeedbackSchema>
export type Operation = z.infer<typeof OperationSchema>
export type ChangeSet = z.infer<typeof ChangeSetSchema>
export type Render = z.infer<typeof RenderSchema>
export type Project = z.infer<typeof ProjectSchema>
export type ProjectSummary = z.infer<typeof ProjectSummarySchema>
