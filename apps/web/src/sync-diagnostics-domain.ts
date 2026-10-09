import { z } from "zod"

const CanonicalSignedIntegerSchema = z.string().regex(/^(?:0|-?[1-9]\d*)$/)
const CanonicalPositiveIntegerSchema = z.string().regex(/^[1-9]\d*$/)
const CanonicalNonnegativeIntegerSchema = z.string().regex(/^(?:0|[1-9]\d*)$/)
const StreamIndexSchema = z.number().int().nonnegative()
const MAX_DIAGNOSTIC_ROWS = 20_000

const CalculationReasonSchema = z.enum([
  "arithmetic_overflow",
  "zero_physical_duration",
  "declared_duration_unverifiable",
  "validation_limit_exceeded",
])

export const ExactValueSchema = z.discriminatedUnion("status", [
  z.object({
    status: z.literal("exact"),
    numerator: CanonicalSignedIntegerSchema,
    denominator: CanonicalPositiveIntegerSchema,
  }),
  z.object({ status: z.literal("unknown"), reason: z.literal("time_maps_missing") }),
  z.object({ status: z.literal("unverifiable"), reason: CalculationReasonSchema }),
])

const ExactRangeSchema = z.object({ start: ExactValueSchema, end: ExactValueSchema })
const SegmentDiagnosticSchema = z.object({
  original_media_start: ExactValueSchema,
  original_media_end: ExactValueSchema,
  session_start: ExactValueSchema,
  session_end: ExactValueSchema,
  confidence: z.string(),
  offset_at_start: ExactValueSchema,
  duration_delta: ExactValueSchema,
  relative_rate_ratio: ExactValueSchema,
  relative_rate_ppm: ExactValueSchema,
})

const CoverageSchema = z.discriminatedUnion("status", [
  z.object({
    status: z.literal("exact"),
    domain: ExactRangeSchema,
    ranges: z.array(ExactRangeSchema),
    gaps: z.array(ExactRangeSchema),
  }),
  z.object({ status: z.literal("unknown"), reason: z.literal("time_maps_missing") }),
  z.object({ status: z.literal("unverifiable"), reason: CalculationReasonSchema }),
])

export const SyncDiagnosticsSchema = z.object({
  package_id: z.string(),
  session_id: z.string(),
  relation: z.literal("metadata_relation_unmeasured_alignment"),
  calculation_status: z.enum(["complete", "unverifiable"]),
  adapter_capability: z.object({
    exact_time_maps: z.boolean(),
    ordinary_probe: z.literal("metadata_only_not_exact"),
    measured_av_alignment: z.literal(false),
    media_mutation: z.literal(false),
  }),
  limits: z.object({
    max_items: CanonicalNonnegativeIntegerSchema,
    max_work: CanonicalNonnegativeIntegerSchema,
    observed_items: CanonicalNonnegativeIntegerSchema,
    observed_work: CanonicalNonnegativeIntegerSchema,
    exceeded: z.boolean(),
  }),
  sources: z.array(
    z.object({
      package_source_id: z.string(),
      display_name: z.string(),
      semantic_role: z.string(),
      editor_binding_available: z.boolean(),
      editor_source_ids: z.array(z.string()),
      bindings: z.array(
        z.object({
          asset_id: z.string(),
          stream_index: StreamIndexSchema,
          package_binding_available: z.boolean(),
          segments: z.array(SegmentDiagnosticSchema),
        }),
      ),
    }),
  ),
  coverage: CoverageSchema,
  overlaps: z.array(
    z.object({
      asset_id: z.string(),
      stream_index: StreamIndexSchema,
      session_range: ExactRangeSchema,
    }),
  ),
  health_notes: z.array(z.string()),
})

export type ExactValue = z.infer<typeof ExactValueSchema>
export type SyncDiagnostics = z.infer<typeof SyncDiagnosticsSchema>
export type ExactCoverage = Extract<SyncDiagnostics["coverage"], { readonly status: "exact" }>

export function exactValueLabel(value: ExactValue): string {
  switch (value.status) {
    case "exact":
      return `${value.numerator}/${value.denominator}`
    case "unknown":
      return `Unknown — ${value.reason.replaceAll("_", " ")}`
    case "unverifiable":
      return `Unverifiable — ${value.reason.replaceAll("_", " ")}`
    default: {
      const exhaustive: never = value
      return exhaustive
    }
  }
}

export type DiagnosticSegmentRow = {
  readonly id: string
  readonly source: string
  readonly stream: string
  readonly sessionRange: string
  readonly offset: string
  readonly durationDelta: string
  readonly relativeRate: string
  readonly relativeRatePpm: string
  readonly relativeRatePpmUnit: "ppm"
  readonly timeUnit: "seconds"
  readonly confidence: string
}

export type DiagnosticRangeRow = {
  readonly id: string
  readonly range: string
  readonly timeUnit: "seconds"
  readonly scope: "coverage_union"
}

export type DiagnosticOverlapRow = {
  readonly id: string
  readonly assetId: string
  readonly streamIndex: number
  readonly range: string
  readonly timeUnit: "seconds"
}

export type UnsupportedMappingRow = {
  readonly id: string
  readonly displayName: string
  readonly reason: string
}

type SourceMapping = {
  readonly source_id: string
  readonly display_name: string
  readonly editor_source_id: string | null
  readonly reason: string | null
}

export function unsupportedMappingRows(
  sources: readonly SourceMapping[],
): readonly UnsupportedMappingRow[] {
  return sources.flatMap((source) =>
    source.editor_source_id === null
      ? [
          {
            id: source.source_id,
            displayName: source.display_name,
            reason: source.reason ?? "Editor mapping unavailable",
          },
        ]
      : [],
  )
}

function exactRangeLabel(range: { readonly start: ExactValue; readonly end: ExactValue }): string {
  return `${exactValueLabel(range.start)} – ${exactValueLabel(range.end)}`
}

export function diagnosticGapRows(coverage: ExactCoverage): readonly DiagnosticRangeRow[] {
  return coverage.gaps.slice(0, MAX_DIAGNOSTIC_ROWS).map((gap, index) => ({
    id: `gap:${index}:${exactRangeLabel(gap)}`,
    range: exactRangeLabel(gap),
    timeUnit: "seconds",
    scope: "coverage_union",
  }))
}

export function diagnosticOverlapRows(
  diagnostics: SyncDiagnostics,
): readonly DiagnosticOverlapRow[] {
  return diagnostics.overlaps.slice(0, MAX_DIAGNOSTIC_ROWS).map((overlap, index) => ({
    id: `overlap:${overlap.asset_id}:${overlap.stream_index}:${index}`,
    assetId: overlap.asset_id,
    streamIndex: overlap.stream_index,
    range: exactRangeLabel(overlap.session_range),
    timeUnit: "seconds",
  }))
}

export function diagnosticSegmentRows(
  diagnostics: SyncDiagnostics,
): readonly DiagnosticSegmentRow[] {
  // This accumulator is intentionally mutable so a malformed response cannot create an unbounded row list.
  const rows: DiagnosticSegmentRow[] = []
  for (const source of diagnostics.sources) {
    for (const [bindingIndex, binding] of source.bindings.entries()) {
      for (const [index, segment] of binding.segments.entries()) {
        if (rows.length === MAX_DIAGNOSTIC_ROWS) return rows
        rows.push({
          id: `${source.package_source_id}:${binding.asset_id}:${binding.stream_index}:${bindingIndex}:${index}`,
          source: source.display_name,
          stream: `${binding.asset_id} · stream ${binding.stream_index}`,
          sessionRange: `${exactValueLabel(segment.session_start)} – ${exactValueLabel(segment.session_end)}`,
          offset: exactValueLabel(segment.offset_at_start),
          durationDelta: exactValueLabel(segment.duration_delta),
          relativeRate: exactValueLabel(segment.relative_rate_ratio),
          relativeRatePpm: exactValueLabel(segment.relative_rate_ppm),
          relativeRatePpmUnit: "ppm",
          timeUnit: "seconds",
          confidence: segment.confidence,
        })
      }
    }
  }
  return rows
}

export function paginateDiagnostics<T>(
  rows: readonly T[],
  visibleCount: number,
): { readonly visible: readonly T[]; readonly remaining: number } {
  return {
    visible: rows.slice(0, visibleCount),
    remaining: Math.max(0, rows.length - visibleCount),
  }
}
