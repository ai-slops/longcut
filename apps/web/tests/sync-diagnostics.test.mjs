import assert from "node:assert/strict"
import test from "node:test"
import {
  diagnosticGapRows,
  diagnosticOverlapRows,
  diagnosticSegmentRows,
  exactValueLabel,
  paginateDiagnostics,
  SyncDiagnosticsSchema,
  unsupportedMappingRows,
} from "../src/sync-diagnostics-domain.ts"

const exact = (numerator, denominator = "1") => ({ status: "exact", numerator, denominator })

const response = {
  package_id: "import-7",
  session_id: "recording-42",
  relation: "metadata_relation_unmeasured_alignment",
  calculation_status: "complete",
  adapter_capability: {
    exact_time_maps: true,
    ordinary_probe: "metadata_only_not_exact",
    measured_av_alignment: false,
    media_mutation: false,
  },
  limits: {
    max_items: "20000",
    max_work: "1000000",
    observed_items: "1",
    observed_work: "1",
    exceeded: false,
  },
  sources: [
    {
      package_source_id: "program",
      display_name: "Program reference",
      semantic_role: "video.program_reference",
      editor_binding_available: true,
      editor_source_ids: ["editor-program"],
      bindings: [
        {
          asset_id: "program-file",
          stream_index: 0,
          package_binding_available: true,
          segments: [
            {
              original_media_start: exact("1"),
              original_media_end: exact("269", "30"),
              session_start: exact("2"),
              session_end: exact("10"),
              confidence: "1",
              offset_at_start: exact("1"),
              duration_delta: exact("1", "30"),
              relative_rate_ratio: exact("240", "239"),
              relative_rate_ppm: exact("1000000", "239"),
            },
          ],
        },
      ],
    },
  ],
  coverage: {
    status: "exact",
    domain: { start: exact("0"), end: exact("10") },
    ranges: [{ start: exact("2"), end: exact("10") }],
    gaps: [{ start: exact("0"), end: exact("2") }],
  },
  overlaps: [],
  health_notes: ["observed_metadata_gap_not_media_corruption"],
}

test("parses the declared sync diagnostics DTO and preserves exact rationals", () => {
  // Given the documented committed-session response.
  // When it crosses the browser boundary.
  const parsed = SyncDiagnosticsSchema.parse(response)
  // Then exact drift remains a fraction string and is never decimalized.
  const row = diagnosticSegmentRows(parsed)[0]
  assert.equal(row?.durationDelta, "1/30")
  assert.equal(row?.relativeRatePpm, "1000000/239")
})

test("labels unavailable exact values by their ordinary unknown reason", () => {
  // Given missing TimeMaps rather than alignment evidence.
  const value = { status: "unknown", reason: "time_maps_missing" }
  // When the value is prepared for compact rendering.
  const label = exactValueLabel(value)
  // Then it stays unknown and is not relabelled synced.
  assert.equal(label, "Unknown — time maps missing")
})

test("bounds the initial diagnostic segment render and loads the next page", () => {
  // Given more segment rows than the initial DOM budget.
  const rows = Array.from({ length: 81 }, (_, index) => ({ id: String(index) }))
  // When initial and expanded pages are selected.
  const initial = paginateDiagnostics(rows, 40)
  const expanded = paginateDiagnostics(rows, 80)
  // Then DOM work is bounded and expansion remains incremental.
  assert.equal(initial.visible.length, 40)
  assert.equal(initial.remaining, 41)
  assert.equal(expanded.visible.length, 80)
  assert.equal(expanded.remaining, 1)
})

test("rejects non-integer and non-canonical exact numerators", () => {
  // Given exact values containing a non-number token or a leading zero.
  const malformed = ["NaN", "01", "+1", "-0"]
  // When each response crosses the diagnostics boundary.
  const results = malformed.map((numerator) =>
    SyncDiagnosticsSchema.safeParse({
      ...response,
      coverage: {
        ...response.coverage,
        domain: { ...response.coverage.domain, start: exact(numerator) },
      },
    }),
  )
  // Then none can enter exact-value rendering.
  assert.equal(
    results.every((result) => !result.success),
    true,
  )
})

test("rejects zero and non-canonical exact denominators", () => {
  // Given denominators that are not canonical positive integers.
  const malformed = ["0", "-1", "01", "+1", "NaN"]
  // When each response crosses the diagnostics boundary.
  const results = malformed.map((denominator) =>
    SyncDiagnosticsSchema.safeParse({
      ...response,
      coverage: {
        ...response.coverage,
        domain: { ...response.coverage.domain, start: exact("1", denominator) },
      },
    }),
  )
  // Then none can enter exact-value rendering.
  assert.equal(
    results.every((result) => !result.success),
    true,
  )
})

test("rejects negative and non-canonical diagnostic counters", () => {
  // Given count strings outside the canonical nonnegative wire format.
  const malformed = ["-1", "01", "+1", "NaN"]
  // When each observed count crosses the diagnostics boundary.
  const results = malformed.map((observed_items) =>
    SyncDiagnosticsSchema.safeParse({
      ...response,
      limits: { ...response.limits, observed_items },
    }),
  )
  // Then invalid counters are rejected before UI pagination.
  assert.equal(
    results.every((result) => !result.success),
    true,
  )
})

test("rejects fractional stream indices in bindings and overlaps", () => {
  // Given malformed stream indices in each DTO location.
  const bindingResponse = structuredClone(response)
  bindingResponse.sources[0].bindings[0].stream_index = 0.5
  const overlapResponse = {
    ...response,
    overlaps: [
      { asset_id: "program-file", stream_index: 1.5, session_range: response.coverage.domain },
    ],
  }
  // When both responses cross the diagnostics boundary.
  const binding = SyncDiagnosticsSchema.safeParse(bindingResponse)
  const overlap = SyncDiagnosticsSchema.safeParse(overlapResponse)
  // Then both uses enforce the same nonnegative-integer contract.
  assert.equal(binding.success, false)
  assert.equal(overlap.success, false)
})

test("preserves huge canonical integers without Number conversion", () => {
  // Given integer strings far beyond JavaScript's safe numeric range.
  const huge = "1234567890123456789012345678901234567890"
  const payload = {
    ...response,
    limits: { ...response.limits, observed_work: huge },
    coverage: {
      ...response.coverage,
      domain: { ...response.coverage.domain, start: exact(`-${huge}`, huge) },
    },
  }
  // When the response crosses the diagnostics boundary.
  const parsed = SyncDiagnosticsSchema.parse(payload)
  // Then the browser retains exact decimal strings unchanged.
  assert.equal(parsed.limits.observed_work, huge)
  assert.equal(exactValueLabel(parsed.coverage.domain.start), `-${huge}/${huge}`)
})

test("caps flattened diagnostic rows at the backend item ceiling", () => {
  // Given a malformed response containing more segments than the backend contract permits.
  const segment = response.sources[0].bindings[0].segments[0]
  const oversized = structuredClone(response)
  oversized.sources[0].bindings[0].segments = Array.from({ length: 20_001 }, () => segment)
  // When rows are prepared for the paginated UI.
  const rows = diagnosticSegmentRows(SyncDiagnosticsSchema.parse(oversized))
  // Then frontend flattening cannot allocate an arbitrary unbounded row list.
  assert.equal(rows.length, 20_000)
})

test("exposes every declared gap and overlap through independent bounded pages", () => {
  // Given more gaps and overlaps than one rendered page.
  const payload = {
    ...response,
    coverage: {
      ...response.coverage,
      gaps: Array.from({ length: 81 }, (_, index) => ({
        start: exact(String(index)),
        end: exact(String(index + 1)),
      })),
    },
    overlaps: Array.from({ length: 81 }, (_, index) => ({
      asset_id: `asset-${index}`,
      stream_index: index,
      session_range: { start: exact(String(index)), end: exact(String(index + 1)) },
    })),
  }
  const parsed = SyncDiagnosticsSchema.parse(payload)
  // When gaps advance to a second page while overlaps remain on their first page.
  const gaps = paginateDiagnostics(diagnosticGapRows(parsed.coverage), 80)
  const overlaps = paginateDiagnostics(diagnosticOverlapRows(parsed), 40)
  // Then both datasets remain accessible with independent bounds and explicit units.
  assert.equal(gaps.visible.length, 80)
  assert.equal(gaps.remaining, 1)
  assert.equal(gaps.visible[0]?.timeUnit, "seconds")
  assert.equal(gaps.visible[0]?.scope, "coverage_union")
  assert.equal(overlaps.visible.length, 40)
  assert.equal(overlaps.remaining, 41)
  assert.equal(overlaps.visible[0]?.timeUnit, "seconds")
  assert.equal(overlaps.visible[0]?.streamIndex, 0)
})

test("assigns explicit units to segment time and drift-rate quantities", () => {
  // Given a parsed segment with exact offset, duration drift, and ppm rate drift.
  const parsed = SyncDiagnosticsSchema.parse(response)
  // When its compact view row is prepared.
  const row = diagnosticSegmentRows(parsed)[0]
  // Then consumers receive unambiguous unit contracts without decimal conversion.
  assert.equal(row?.timeUnit, "seconds")
  assert.equal(row?.relativeRatePpmUnit, "ppm")
})

test("keeps segment row IDs unique across disjoint bindings to the same stream", () => {
  // Given two valid binding entries that target the same asset and stream.
  const payload = structuredClone(response)
  payload.sources[0].bindings.push(structuredClone(payload.sources[0].bindings[0]))
  // When segment view rows are flattened.
  const rows = diagnosticSegmentRows(SyncDiagnosticsSchema.parse(payload))
  // Then binding ordinal ownership prevents React-key collisions.
  assert.equal(rows.length, 2)
  assert.notEqual(rows[0]?.id, rows[1]?.id)
})

test("keeps every unsupported source reachable through bounded pagination", () => {
  // Given more unsupported source mappings than one visible page.
  const sources = Array.from({ length: 81 }, (_, index) => ({
    source_id: `source-${index}`,
    display_name: `Source ${index}`,
    editor_source_id: null,
    reason: index === 80 ? null : `reason-${index}`,
  }))
  // When presentation rows are built and the initial page is selected.
  const rows = unsupportedMappingRows(sources)
  const page = paginateDiagnostics(rows, 40)
  // Then no source is discarded and the DOM remains initially bounded.
  assert.equal(rows.length, 81)
  assert.equal(rows[80]?.reason, "Editor mapping unavailable")
  assert.equal(page.visible.length, 40)
  assert.equal(page.remaining, 41)
})
