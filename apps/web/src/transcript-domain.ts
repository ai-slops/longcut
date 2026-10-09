import { z } from "zod"

const FrameSchema = z.string().regex(/^\d+$/)

export const TranscriptCueSchema = z.object({
  id: z.string().min(1),
  start_frame: FrameSchema,
  end_frame: FrameSchema,
  text: z.string(),
})

const TranscriptPageBaseSchema = z.object({
  id: z.string().min(1),
  source_id: z.string().min(1),
  filename: z.string(),
  created_at: z.string(),
  frame_rate: z.number().positive(),
  cue_count: z.number().int().nonnegative(),
  cues: z.array(TranscriptCueSchema),
  next_cursor: z.string().nullable(),
})

export const TranscriptPageSchema = z.discriminatedUnion("origin", [
  TranscriptPageBaseSchema.extend({ origin: z.literal("subtitle_import") }),
  TranscriptPageBaseSchema.extend({
    origin: z.literal("local_asr"),
    asr: z.object({
      engine: z.literal("whisper.cpp"),
      model: z.string(),
      requested_language: z.enum(["ko", "en", "auto"]),
      result_language: z.string().nullable(),
    }),
  }),
])

export const OptionalTranscriptPageSchema = TranscriptPageSchema.nullable()

export type TranscriptCue = z.infer<typeof TranscriptCueSchema>
export type TranscriptPage = z.infer<typeof TranscriptPageSchema>

export type TranscriptProvenance =
  | { readonly kind: "subtitle_import"; readonly filename: string }
  | {
      readonly kind: "local_asr"
      readonly engine: "whisper.cpp"
      readonly model: string
      readonly requestedLanguage: "ko" | "en" | "auto"
      readonly resultLanguage: string | null
      readonly verification: "unverified"
    }

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

export function transcriptProvenance(page: TranscriptPage): TranscriptProvenance {
  switch (page.origin) {
    case "subtitle_import":
      return { kind: "subtitle_import", filename: page.filename }
    case "local_asr":
      return {
        kind: "local_asr",
        engine: page.asr.engine,
        model: page.asr.model,
        requestedLanguage: page.asr.requested_language,
        resultLanguage: page.asr.result_language,
        verification: "unverified",
      }
    default: {
      const exhaustive: never = page
      return exhaustive
    }
  }
}
