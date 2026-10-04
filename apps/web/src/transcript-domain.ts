import { z } from "zod"

const FrameSchema = z.string().regex(/^\d+$/)

export const TranscriptCueSchema = z.object({
  id: z.string().min(1),
  start_frame: FrameSchema,
  end_frame: FrameSchema,
  text: z.string(),
})

export const TranscriptPageSchema = z.object({
  id: z.string().min(1),
  source_id: z.string().min(1),
  filename: z.string(),
  created_at: z.string(),
  frame_rate: z.number().positive(),
  cue_count: z.number().int().nonnegative(),
  origin: z.literal("subtitle_import"),
  cues: z.array(TranscriptCueSchema),
  next_cursor: z.string().nullable(),
})

export const OptionalTranscriptPageSchema = TranscriptPageSchema.nullable()

export type TranscriptCue = z.infer<typeof TranscriptCueSchema>
export type TranscriptPage = z.infer<typeof TranscriptPageSchema>

export type CueSelection = {
  readonly sourceFrame: number
  readonly inFrame: number
  readonly outFrame: number
}

export function cueSelection(cue: TranscriptCue): CueSelection {
  return {
    sourceFrame: Number(cue.start_frame),
    inFrame: Number(cue.start_frame),
    outFrame: Number(cue.end_frame),
  }
}

export function transcriptRequestMatches(
  currentRequestVersion: number,
  responseRequestVersion: number,
): boolean {
  return currentRequestVersion === responseRequestVersion
}
