# EditSessionPackage 0.1 metadata contract

Implemented 2026-09-22 as the standalone `longcut-session-contract` Rust crate.
This completes the **metadata schema/validator prerequisite**, not M6's complete
media importer, reconstruction UI, or real-broadcast acceptance.

## What is validated

The package describes completed **source recordings**, not an edited project,
command history, OBS scene graph, or replayable producer state. Six file types form
the metadata boundary:

| File | Typed model | Contents |
| --- | --- | --- |
| `manifest.json` | `Manifest` | Contract/version/completion, session ID/duration, reference and metadata paths |
| `sources.json` | `Registry` | Semantic sources, assets, streams and optional audio channel selections |
| `time-maps.json` | `TimeMaps` | Exact media/session segment endpoints and confidence |
| `visual-composition.json` | `VisualComposition` | Canvas, independent source contributions, normalized geometry and coverage |
| `audio-mix.json` | `AudioMix` | Independent stem contributions, audibility, gain/pan and coverage |
| `media-health.json` | `MediaHealth` | Asset/stream availability, damage and explicit gaps |

Only the manifest name is fixed. Other paths come from the manifest. The
[published schema bundle](../schemas/edit-session-package/schema-bundle.json)
contains six JSON Schema 2020-12 documents generated directly from Rust types.
`schemas()["sources"]`, for example, is the schema root for the registry file.
Schema drift is tested against generated output; do not edit the bundle manually.

All fields use snake_case. The public package wire model is separate from the
internal [timebase library](timebase.md). Media timestamps use string `ticks` plus
numeric u32 `timebase_num`/`timebase_den`. Session endpoints use string
`monotonic_ns`. Duration uses string `ticks`/`timescale`; it must resolve to positive,
exact i64 nanoseconds. Optional stream rational metadata uses string
`numerator`/`denominator`. No i64 value crosses JSON as a number.

The schema layer checks structure. The semantic layer checks:

- Canonical `0.1.x` compatibility, nonempty IDs, distinct source/asset/stream IDs.
- Source → asset → stream/channel references and video/audio/AV kind consistency.
- Duplicate or intersecting channel bindings; disjoint channel subsets remain valid.
- Positive exact ranges, nonnegative session times, session bounds and confidence.
- Piecewise map ordering/overlap through `longcut-timebase`; different streams may
  share session ranges and different source clocks are compared as rational time.
- Program reference video **and** audio coverage across the complete session, even
  when split among assets. Short or interrupted reference coverage degrades metadata.
- Visual/mix temporal coverage and overlap, independent contribution sources,
  finite normalized geometry/crop/opacity, finite gain and bounded pan.
- Health coverage over mapped ranges and missing/timestamp-gap declarations between
  segments, including collisions between asset-wide and stream-specific intervals.
  Independent sources may legitimately start later or finish earlier than the session;
  their leading/trailing absence is not fabricated as a time map. Program reference
  is separately checked for complete-session coverage.
- Namespaced extensions and package-relative path policy.

## Result semantics

The report intentionally uses **`metadata_conformance`**, not an unqualified package
conformance claim. Every report includes `media_verification: "not_performed"`.

| Result | Meaning | CLI exit |
| --- | --- | --- |
| `conformant` | Six-element metadata passes this boundary's structural and semantic checks | 0 |
| `degraded` | Recoverable missing tracks/reference/independent sources/coverage or declared damaged media | 1 |
| `rejected` | Unparseable/core-missing input, unsupported version, invalid references/ranges, unsafe paths or inspection limits | 2 |

Missing optional track files degrade the result. Present but malformed optional files
are rejected, not silently treated as absent. A missing or malformed registry rejects
the package. A Program-only package remains usable in degraded mode. Rejected reports
clear all capabilities and include stable codes, severity and diagnostic locations.
Semantic locations use canonical logical file names even when a manifest chooses
different filenames; filesystem/parse errors retain the supplied relative filename.

Capabilities describe **declared metadata**, not online/decoded media. Reconstruction
booleans are conservative full-track eligibility: valid mapping, full temporal
coverage, at least one independent contribution, and `coverage: complete` on every
interval. Explicit `partial` or omitted/unknown coverage is valid metadata but does
not claim full reconstruction. Health metadata can be complete while identifying
damage; callers must inspect affected intervals rather than treat capability flags
as permission to render damaged media.

For example, the complete synthetic fixture returns:

```json
{
  "metadata_conformance": "conformant",
  "media_verification": "not_performed",
  "capabilities": {
    "program_reference": true,
    "independent_video_sources": true,
    "independent_audio_sources": true,
    "session_time_map": true,
    "semantic_visual_reconstruction": true,
    "semantic_audio_reconstruction": true,
    "media_health_ranges": true
  },
  "diagnostics": []
}
```

**The fixture intentionally has no media files.** This is correct metadata-only
output; it must never be promoted to full media conformance without further checks.

## Local inspection and safety limits

The synchronous loader reads immutable, local, completed metadata. It does not open
media, fetch URLs, execute extensions, modify packages, or write project state.
Use it outside an async runtime's request thread when integrating later.

- Metadata and media URI strings must be literal relative paths with normal segments.
  No absolute paths, `.`/`..`, backslashes, percent encoding, URI schemes, query or
  fragment syntax, or control characters are supported in this initial reader.
- Referenced metadata must be a regular file, with no symlink path components.
  The explicitly selected package root is trusted and must not be concurrently changed.
- Reads are bounded to **4 MiB per metadata file**; bounded reads also check growth.
- Before semantic loops, a conservative aggregate policy caps **20,000 metadata
  items** and **1,000,000 estimated pair/expansion work units**. This bounds channel
  intersections, TimeMap comparisons and health cross-product expansion. Oversized
  packages fail with `VALIDATION_LIMIT_EXCEEDED`, without claiming malformed content.
- These are implementation safety limits, not permanent format limits. Large-track
  streaming/indexed validation remains future work; do not raise limits blindly.

Unknown extensions are retained only under root-file `extensions` objects with dotted
namespace keys, for example `org.producer.feature`. Their contents have no executable
meaning. Unknown core fields reject input. Only `0.1.x` patches are supported; `0.2`
is not assumed compatible while the format remains pre-1.0.

## Run

With a current stable Rust toolchain:

```bash
cargo run --locked -p longcut-session-contract --bin session-inspect -- --help
cargo run --locked -p longcut-session-contract --bin session-inspect -- \
  inspect fixtures/edit-session-packages/complete
cargo run --locked -p longcut-session-contract --bin session-inspect -- \
  inspect fixtures/edit-session-packages/program-only
```

The Program-only command intentionally exits 1. In this workspace, use the pinned
Docker toolchain, matching the main server build rather than the older timebase-only
Rust 1.85 image:

```bash
docker build --target check -t longcut-session-validation \
  -f crates/session-contract/Dockerfile .
docker run --rm longcut-session-validation --help
docker run --rm longcut-session-validation inspect fixtures/edit-session-packages/complete
docker run --rm longcut-session-validation inspect fixtures/edit-session-packages/program-only
docker run --rm longcut-session-validation schema
```

The image includes synthetic fixtures. The workspace's Docker daemon is remote:
host bind mounts are not the agent checkout. To inspect another package, transfer it
to a disposable container with `docker cp`, not a guessed remote bind mount.

Library consumers can deserialize the public `model` types into `Package`, call
`validate(&package)`, or use `inspect(&Path)` for the bounded directory reader.
Deserialization alone is not semantic validation; do not persist rejected input.
`load(&Path)` now returns an accepted `LoadedPackage`, whose `into_parts()` yields
the typed package and report. Rejected metadata returns only the rejection report.
This gate is used by the separate [actual-media preflight](session-media.md).

## Verified evidence

- **22 contract tests**, including a 23-case file-mutation fixture matrix.
- Real JSON Schema validation of all six fixture files, closed-core rejection and
  exact published/generated schema equality.
- Loader tests for missing/malformed/oversized files and symlinks; CLI output and
  exit codes; exact-time overlap/discontinuity; channel/health/coverage boundaries.
- Three review-discovered incorrect acceptance/capability cases reproduced as failing
  tests before fixes; metadata amplification now has an early-rejection regression.
- Package formatting and Clippy with warnings denied passed.
- Full root Docker `check` passed: server release build, all-workspace formatting,
  Clippy and **93 Rust tests** (55 server, 22 contract, 16 timebase).
- Manual CLI execution observed help, complete/conformant, Program-only/degraded,
  and missing-directory/rejected output. Host Rust LSP is unavailable; no LSP-clean
  claim is made. No browser UI or live model was exercised in this increment.

Module responsibilities remain separated into wire models, generated schemas,
metadata loading, reports and semantic validators. New files remain below 250 pure
LOC; the registry validator is at 245 and should split asset/source validation before
future growth. Opaque extension JSON is never interpreted by semantic code.

## Follow-up and remaining scope

The separate [media preflight](session-media.md) now implements actual file access,
size/SHA-256 comparison, bounded ffprobe stream checks and exact packet-envelope
TimeMap checks without changing this
metadata-only report or CLI. Its additional loader tests bring contract coverage to
24 tests; the 22/93 counts above remain evidence for the original metadata increment.

Separate checksum-manifest files, full decode, continuous playable coverage/sampled AV alignment,
package import transactions, persistent SourceSession storage/migration, HTTP/UI
wiring, reconstruction hints and long-session acceptance remain unfinished. Do not
mark M6 or Stage 1 alpha complete from metadata, file/header or packet-extent preflight alone.
