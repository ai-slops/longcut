# Completed-session metadata fixtures

These are **synthetic metadata only**. No broadcast media or sample media files are
included. `available` is a producer declaration, not a decoder verification result.

- `complete/`: all six contract elements; reference AV plus independent video/audio,
  stream/channel bindings, exact 10-second maps, visual/mix/health intervals and an
  unknown optional namespaced extension. Expected metadata result: conformant.
- `program-only/`: openable reference media registry without reconstruction tracks.
  Expected metadata result: degraded, not rejected.
- `cases.json`: 23 single-field mutations applied to isolated copies of `complete/`
  by `crates/session-contract/tests/files.rs`. Each case records a file, JSON pointer,
  replacement, expected conformance and optional machine diagnostic code. Covers
  versions, in-progress input, integer precision, corrupt/missing declarations,
  invalid references/channels/geometry/mix, gaps and unsafe paths.

Additional typed fixture tests cover adjacent/overlapping intervals, stream-scoped
health versus asset-wide health, discontinuities, explicit unknown coverage, channel
aliasing, short Program reference, and metadata work-budget rejection. Filesystem
tests generate missing, malformed, oversized and symlinked metadata independently.

The metadata tests never download or decode the declared media, execute extensions, write
user projects or require a running broadcast application.

For real-file tests, `crates/session-media` generates small H.264/AAC recordings in
isolated temporary directories and writes matching declarations. Its
`make_fixture` example can create the same synthetic package for manual inspection;
see [media preflight](../../docs/session-media.md). No binary fixture is committed here.
