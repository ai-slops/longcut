import { z } from "zod"
import type { StoryActionInput, StoryCommand } from "./command.ts"

const IdSchema = z.string().min(1)
const VersionSchema = z.string().regex(/^\d+$/)
const TargetFramesSchema = z
  .string()
  .regex(/^[1-9]\d*$/)
  .refine((value) => /^[1-9]\d*$/.test(value) && BigInt(value) <= 2_147_483_647n)

export const StoryMomentRefSchema = z
  .object({ moment_id: IdSchema, version: VersionSchema })
  .strict()

export const BeatSchema = z
  .object({
    id: IdSchema,
    title: z.string(),
    purpose: z.string(),
    target_frames: TargetFramesSchema.nullable(),
    moment_refs: z.array(StoryMomentRefSchema),
  })
  .strict()

export const ChapterSchema = z
  .object({
    id: IdSchema,
    title: z.string(),
    purpose: z.string(),
    beats: z.array(BeatSchema),
  })
  .strict()

export const StorySchema = z
  .object({ version: VersionSchema, chapters: z.array(ChapterSchema) })
  .strict()
  .default({ version: "0", chapters: [] })

export const StoryVersionResponseSchema = z
  .object({
    project_id: IdSchema,
    version: VersionSchema,
    snapshot: StorySchema.removeDefault(),
  })
  .strict()

export type StoryMomentRef = z.infer<typeof StoryMomentRefSchema>
export type Beat = z.infer<typeof BeatSchema>
export type Chapter = z.infer<typeof ChapterSchema>
export type Story = z.infer<typeof StorySchema>
export type StoryVersionResponse = z.infer<typeof StoryVersionResponseSchema>

export const storyLimits = {
  chapters: 64,
  beats: 512,
  beatsPerChapter: 128,
  refsPerBeat: 64,
} as const

export function createStoryCommand(
  expectedStoryVersion: string,
  action: StoryActionInput,
): StoryCommand {
  return { type: "story", action: { ...action, expected_story_version: expectedStoryVersion } }
}

export function validateTargetFrames(value: string): string | null | false {
  if (value === "") return null
  return TargetFramesSchema.safeParse(value).success ? value : false
}

export function validateStoryText(
  value: string,
  field: "title" | "purpose",
): { readonly valid: boolean; readonly bytes: number } {
  const bytes = new TextEncoder().encode(value).byteLength
  const valid = field === "title" ? value.trim().length > 0 && bytes <= 200 : bytes <= 2_000
  return { valid, bytes }
}

type FrameRange = { readonly in_frame: string; readonly out_frame: string }

export function budgetSummary(
  target: string | null,
  ranges: readonly FrameRange[],
): { readonly target: string | null; readonly actual: string; readonly delta: string | null } {
  const actual = ranges.reduce(
    (total, range) => total + BigInt(range.out_frame) - BigInt(range.in_frame),
    0n,
  )
  if (target === null) return { target, actual: actual.toString(), delta: null }
  const difference = actual - BigInt(target)
  return {
    target,
    actual: actual.toString(),
    delta: difference > 0n ? `+${difference}` : difference.toString(),
  }
}

type MomentHead = {
  readonly id: string
  readonly version: string
  readonly review_status: "draft" | "reviewed" | "rejected"
  readonly archived: boolean
}

export function linkableMomentHeads(
  moments: readonly MomentHead[],
  refs: readonly StoryMomentRef[],
): { readonly available: readonly StoryMomentRef[]; readonly pinned: readonly StoryMomentRef[] } {
  const linked = new Set(refs.map((ref) => ref.moment_id))
  return {
    available: moments
      .filter(
        (moment) =>
          moment.review_status === "reviewed" && !moment.archived && !linked.has(moment.id),
      )
      .map((moment) => ({ moment_id: moment.id, version: moment.version })),
    pinned: refs,
  }
}

export function assignmentAvailability(
  scene: { readonly locked: boolean; readonly beat_id: string | null },
  nextBeatId: string | null,
): { readonly enabled: boolean; readonly reason: string | null } {
  if (!scene.locked || scene.beat_id === nextBeatId) return { enabled: true, reason: null }
  return { enabled: false, reason: "Unlock this scene before changing its beat." }
}

export function editableStoryDraft<T>(
  openedVersion: string,
  currentVersion: string,
  fields: T,
): { readonly stale: boolean; readonly fields: T } {
  return { stale: openedVersion !== currentVersion, fields }
}

export type StoryDraft<T extends Record<string, unknown>> = {
  readonly openedVersion: string
  readonly baseline: T
  readonly fields: T
}

function sameFields<T extends Record<string, unknown>>(left: T, right: T): boolean {
  const keys = Object.keys(left)
  return keys.length === Object.keys(right).length && keys.every((key) => left[key] === right[key])
}

export function reconcileStoryDraft<T extends Record<string, unknown>>(
  draft: StoryDraft<T>,
  currentVersion: string,
  currentFields: T,
): StoryDraft<T> {
  if (draft.openedVersion === currentVersion) return draft
  const pristine = sameFields(draft.fields, draft.baseline)
  const matchesServer = sameFields(draft.fields, currentFields)
  if (!pristine && !matchesServer) return draft
  return { openedVersion: currentVersion, baseline: currentFields, fields: currentFields }
}
