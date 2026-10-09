# Source-session sync diagnostics API

`GET /api/v1/projects/{project}/source-sessions/{session}/sync-diagnostics`

This read-only endpoint projects the immutable `SourceSessionRecord` into exact
metadata diagnostics. It neither probes nor rewrites media. Access follows the
existing active-project source-session membership rule: an import hidden by undo
returns `404`, while redo makes the same preserved record visible again.

## Interpretation

- `relation` is always `metadata_relation_unmeasured_alignment`. TimeMap metadata
  describes a declared relation; it is not a measured audio/video alignment claim.
- `adapter_capability.measured_av_alignment` and `media_mutation` are always `false`.
- `ordinary_probe: metadata_only_not_exact` means an ordinary media probe cannot
  establish this exact mapping. Probe observations must not be relabeled as exact.
- `exact_time_maps` only reports whether the preserved package supplied TimeMaps.
- All time, offset, drift, ratio, and ppm quantities are reduced rationals whose
  numerator and denominator are decimal JSON strings. Consumers must use `BigInt`
  or a rational library, never `Number` or floating-point parsing.
- `confidence` is a string projection of the package's declared confidence; it is
  not physical verification evidence.

An exact quantity has this shape:

```json
{ "status": "exact", "numerator": "1", "denominator": "30" }
```

An unavailable or unsafe calculation is explicit:

```json
{ "status": "unverifiable", "reason": "arithmetic_overflow" }
```

Possible calculation reasons are `arithmetic_overflow`,
`zero_physical_duration`, `declared_duration_unverifiable`, and
`validation_limit_exceeded`. Missing TimeMaps produce coverage
`{"status":"unknown","reason":"time_maps_missing"}` and empty segment arrays.

## Arithmetic

For each TimeMap segment, in exact seconds:

- `offset_at_start = session_start - original_media_start`
- `duration_delta = session_duration - physical_media_duration`
- `relative_rate_ratio = session_duration / physical_media_duration`
- `relative_rate_ppm = (relative_rate_ratio - 1) * 1,000,000`

Calculations use checked `i128` intermediates and reduced fractions. Failure is
reported on the affected value rather than panicking, rounding, or emitting a
floating-point approximation. Media endpoint clocks may differ per stream.

## Coverage and overlap

Coverage uses the union of all TimeMap session ranges, clamped to the half-open
declared session domain `[0, manifest.duration)`. `coverage.ranges` is the merged
union; `coverage.gaps` is its complement in that domain. Adjacent ranges merge.
The note `observed_metadata_gap_not_media_corruption` states that a TimeMap gap is
observed metadata health information, not automatic proof of corrupt media.

Overlap detection is grouped by `(asset_id, stream_index)`. Simultaneous ranges on
different streams are expected and never reported as overlaps.

Work is bounded using the session-contract ceilings: 20,000 diagnostic items and
1,000,000 estimated work units. `observed_items` is a pre-materialization,
saturating output bound: source rows, binding rows, projected segment rows emitted
through every matching binding, plus conservative bounds for coverage ranges, gaps,
and per-stream overlaps. Shared-stream bindings therefore count every duplicated
segment row that the response would emit. The response exposes limits and observed
counts as decimal strings. When exceeded, `calculation_status` is `unverifiable`,
sources are omitted, and coverage reports `validation_limit_exceeded`.

`calculation_status` is also `unverifiable` whenever coverage or any nested segment
quantity is unverifiable. It is `complete` when all performed calculations succeed;
an explicitly `unknown` optional capability such as missing TimeMaps does not claim
failure and does not by itself change the top-level status.

## Frontend fixture

```json
{
  "package_id": "import-7",
  "session_id": "recording-42",
  "relation": "metadata_relation_unmeasured_alignment",
  "calculation_status": "complete",
  "adapter_capability": {
    "exact_time_maps": true,
    "ordinary_probe": "metadata_only_not_exact",
    "measured_av_alignment": false,
    "media_mutation": false
  },
  "limits": {
    "max_items": "20000",
    "max_work": "1000000",
    "observed_items": "7",
    "observed_work": "2",
    "exceeded": false
  },
  "sources": [{
    "package_source_id": "program",
    "display_name": "Program reference",
    "semantic_role": "video.program_reference",
    "editor_binding_available": true,
    "editor_source_ids": ["editor-program"],
    "bindings": [{
      "asset_id": "program-file",
      "stream_index": 0,
      "package_binding_available": true,
      "segments": [{
        "original_media_start": {"status":"exact","numerator":"1","denominator":"1"},
        "original_media_end": {"status":"exact","numerator":"269","denominator":"30"},
        "session_start": {"status":"exact","numerator":"2","denominator":"1"},
        "session_end": {"status":"exact","numerator":"10","denominator":"1"},
        "confidence": "1",
        "offset_at_start": {"status":"exact","numerator":"1","denominator":"1"},
        "duration_delta": {"status":"exact","numerator":"1","denominator":"30"},
        "relative_rate_ratio": {"status":"exact","numerator":"240","denominator":"239"},
        "relative_rate_ppm": {"status":"exact","numerator":"1000000","denominator":"239"}
      }]
    }]
  }],
  "coverage": {
    "status": "exact",
    "domain": {
      "start": {"status":"exact","numerator":"0","denominator":"1"},
      "end": {"status":"exact","numerator":"10","denominator":"1"}
    },
    "ranges": [{
      "start": {"status":"exact","numerator":"2","denominator":"1"},
      "end": {"status":"exact","numerator":"10","denominator":"1"}
    }],
    "gaps": [{
      "start": {"status":"exact","numerator":"0","denominator":"1"},
      "end": {"status":"exact","numerator":"2","denominator":"1"}
    }]
  },
  "overlaps": [],
  "health_notes": ["observed_metadata_gap_not_media_corruption"]
}
```

The panel should label these values as **declared metadata diagnostics**, preserve
the unknown/unverifiable states, and avoid any “synced”, “verified alignment”, or
“media repaired” claim.
