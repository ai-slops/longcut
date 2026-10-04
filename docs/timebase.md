# Exact timebase library

`crates/timebase` provides `longcut-timebase`, the pure time-model prerequisite for
the revised design's §§13.3–13.4 and M0-E. It is deliberately **not yet imported by
the editor/server** and does not migrate projects or declare package conformance.

## Public concepts

| Type | Meaning |
| --- | --- |
| `TimeBase` | Positive reduced rational seconds per source tick |
| `SourceTime` | Signed i64 tick count and explicit `TimeBase` |
| `SessionTime` | Signed i64 nanoseconds in the common session coordinate space |
| `OutputRate` | Positive reduced rational frames per second, e.g. 60000/1001 |
| `OutputFrame` | Signed i64 frame index; caller supplies the output clock |
| `StreamKey` | Non-empty opaque asset and stream identities |
| `MediaPoint` | Source timestamp plus its stream identity |
| `SegmentSpec` / `Segment` | Input endpoints / validated positive half-open mapping |
| `TimeMap` | Validated piecewise mappings separated by asset/stream |

Use constructors or serde deserialization to enter the validated boundary. A map
rejects overlaps in either clock and reversed temporal ordering for one stream;
different streams may occupy the same session interval. Input ordering is not a
requirement: pairwise validation checks temporal consistency independently of vector
order. Adjacent ranges are valid; the exclusive final endpoint and gaps return `None`.

Segment endpoints define an affine map, so offsets, clock drift and discontinuities
are explicit. Lookup accepts a point with a different source timebase and compares
physical rational time, not raw tick counts. Inverse mapping returns ticks in the
segment's **starting** source timebase.

## Precision and rounding

Intermediates use reduced, checked i128 rational arithmetic, not floating-point
seconds. Failures return `TimeError` / `MapError`, not panic or saturation. This is
bounded arithmetic, not arbitrary precision; sufficiently extreme rational products
can return `Overflow` even when a later mathematical cancellation might fit.

`SourceTime::compare(other)` compares physical times across source clocks directly
and returns `Result<Ordering, TimeError>`. Unlike conversion through session
nanoseconds, it preserves subnanosecond differences and supports timestamps outside
the representable i64 nanosecond range. This is used by the media packet-envelope
validator; representation equality (`PartialEq`) remains distinct from physical-time
equality across different clocks.

Every conversion requires one explicit policy:

- `Exact`: reject fractional destination ticks with `Inexact`.
- `Floor`: toward negative infinity.
- `Ceil`: toward positive infinity.
- `Nearest`: nearest integer, ties away from zero in the **destination** coordinate.

Rounding happens once, after the complete affine expression. Membership is checked
before rounding; a rounded value may land on an exclusive endpoint. Callers selecting
playable frames must use appropriate rounding and enforce the output range rather
than assuming rounding itself clamps or selects a segment.

Nanoseconds cannot represent every 48kHz sample or 60000/1001 frame exactly; choose
rounding deliberately. OutputRate conversion changes clocks with a common origin.
It does **not** encode edited clip order, source-span selection, or render lineage.

## Internal JSON representation

Every i64 component is a decimal JSON **string**, including rational components:

```json
{
  "ticks": "9007199254740993",
  "time_base": { "numerator": "1", "denominator": "1000000000" }
}
```

`SessionTime` and `OutputFrame` are transparent string scalars. A TimeMap serializes
as `{ "segments": [...] }`; each segment contains `stream: {asset, stream}`,
`media_start`, `media_end`, `session_start`, `session_end`. Deserializing `Segment`
or `TimeMap` runs the same invariants as their constructors. `SegmentSpec` is only
an unvalidated input object; pass it to `Segment::new` before use.

This is a library wire form, **not the published EditSessionPackage schema**. A future
adapter must map package asset IDs/stream indexes and prescribed field names, retain
segment IDs/confidence/provenance, validate session duration/health coverage, and
generate shared client types. Never coerce these strings through JavaScript `Number`.

## Use and validation

With Rust installed:

```bash
cargo test --locked -p longcut-timebase
cargo fmt --package longcut-timebase -- --check
cargo clippy --locked -p longcut-timebase --all-targets --all-features -- -D warnings
cargo run --locked -p longcut-timebase --example time_map
```

In this workspace, Rust is available through Docker, without host bind mounts:

```bash
docker build -t longcut-timebase-validation:latest -f crates/timebase/Dockerfile .
docker run --rm longcut-timebase-validation:latest
```

The example imports the library publicly, maps a negative-origin 48kHz source through
drift, maps back, queries a gap, rejects a reversed segment, and preserves a value
above JavaScript's safe integer range. Tests cover fractional rates, explicit negative
rounding, conversion overflow, JSON validation, stream isolation, source overlap,
reversed order, differing clocks, discontinuities, final-coordinate rounding and
full-width i64 session spans.

The library is a small correctness foundation. The separate
[session contract crate](session-contract.md) now provides generated package schemas
and metadata mapping validation. Randomized properties, indexed large-track lookups,
real VFR/frame PTS ingestion, audio sync measurement and persisted-project integration
remain separate work.
