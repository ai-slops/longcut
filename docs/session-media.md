# Completed-session media preflight

`crates/session-media` adds read-only **file integrity, stream-header and packet-extent verification**
after the [metadata contract](session-contract.md) gate. It does not import projects,
fully decode recordings, prove continuous playable coverage, or measure AV synchronization.

## Verified scope

For each declared asset, the verifier:

1. Resolves a literal package-relative regular file, rejecting symlink components.
2. Streams the complete file through SHA-256 using a 256 KiB buffer; records actual
   byte count and digest and checks size/mtime before and after inspection.
3. Compares optional `size_bytes` and `content_hash` declarations. Supported hash
   syntax is **`sha256:<64 hexadecimal digits>`**. Other algorithms/representations
   reject preflight rather than silently skipping the check. When absent, the result
   is `not_declared`; the actual size and SHA-256 are still recorded.
4. Runs bounded `ffprobe` against the local file only if integrity declarations match.
5. Checks every declared stream index, media kind, canonical codec name, and supplied
   width/height/sample rate/channel count/channel layout/time base. Video must expose
   positive dimensions; audio must expose positive sample rate and channel count.
   Rational time bases are compared exactly, not as floating-point seconds.
6. Records the ffprobe version and per-asset diagnostic codes.
7. For assets with declared TimeMap segments and matching stream metadata, scans
   packet PTS/duration to EOF and compares declared media endpoints against each
   stream's observed `[min(PTS), max(PTS + duration))` envelope using exact rational
   clocks. Equal start/end boundaries are allowed; even a subnanosecond overrun fails.

Additional undeclared streams are not rejected. Declared frame rate and orientation
are listed under `unchecked_fields`: inferring frame-rate correctness for VFR or
rotation correctness from a header alone would overstate the evidence.
`stream_metadata: passed` refers only to the checked fields above.

## Report and exit codes

`metadata` contains the **unchanged** metadata-only report, including its original
`media_verification: not_performed`. Actual checks live in the outer report's
`assets` array and `preflight_conformance`. Do not read nested metadata capabilities
as proof that damaged/missing media can be rendered.

A successful packet-bound report includes the following fields. The bounds status
varies with evidence; full decode and alignment are still not performed:

```json
{
  "verification_scope": "file_integrity_stream_probe_and_packet_bounds",
  "full_decode_verification": "not_performed",
  "time_map_media_bounds_verification": "passed",
  "time_map_bounds_method": "packet_pts_envelope",
  "alignment_verification": "not_performed"
}
```

Each asset has separate `file`, `size`, `checksum`, `probe`, and `stream_metadata`
checks: `passed`, `failed`, `not_performed`, or `not_declared`. Optional observations
include string `actual_size_bytes`, hexadecimal `actual_sha256`, and `ffprobe_version`.
No 64-bit size is serialized through a JavaScript number.

The new per-asset `time_map_bounds` and aggregate `time_map_media_bounds_verification`
add `unverifiable` for incomplete evidence. `time_map_segments` identifies checked
entries by their original zero-based `time-maps.json/segments` index and stream index.
`packet_bounds` records string start/end PTS, time base and packet count, plus
`timing_complete`. Signed timestamps and counters never pass through floating point.

- `failed`: a declared endpoint exceeds a complete observed packet envelope;
  diagnostic `TIME_MAP_RANGE_OUT_OF_BOUNDS` identifies the input segment.
- `unverifiable`: PTS/duration/time base is unavailable, a duration is nonpositive,
  arithmetic overflows, or scanning fails/times out. No header-duration fallback is
  invented. Diagnostics distinguish `TIME_MAP_BOUNDS_UNVERIFIABLE` from
  `PACKET_SCAN_UNVERIFIABLE`.
- `not_performed`: prerequisite file/hash/header checks failed or work was interrupted
  before the scan. Partial success elsewhere cannot promote this to `passed`.
- `not_declared`: the asset has no declared TimeMap segments, so no packet scan runs.

Aggregation prioritizes failed, unverifiable, not-performed, then passed evidence;
not-declared is neutral. A bounds failure or inability to verify degrades preflight.
Negative PTS and packet reordering are retained. A summary with `timing_complete:
false` contains partial observations, not proven extents.

- **0 / conformant:** metadata and checks within the declared preflight scope pass.
- **1 / degraded:** metadata is degraded, or a file is absent, mismatched, unreadable,
  unsupported by the probe, corrupt enough to fail probing, or exceeds a deadline.
- **2 / rejected:** metadata rejected before file access; unsafe media path,
  unsupported checksum declaration, unavailable ffprobe, invalid timeout, or CLI error.
- **130 / interrupted:** CLI Ctrl+C requests cancellation, awaits cleanup, and emits
  the partial report with `cancelled: true` before exiting.

An ffprobe failure does not necessarily mean corruption: a valid container outside
the supported standalone-file policy can also fail. See the issue code/message.
Passing preflight is not full `EditSessionPackage` media conformance or an import
authorization. Packet envelopes do not prove availability of every instant inside
the envelope, continuity, frame/sample decodability, edit-list playback behavior or
AV alignment. In particular, negative AAC priming packets are evidence of encoded
PTS, not a claim that their samples are audible. These remain separate checks.

## Safety and cancellation

- Originals and metadata are only read. No chmod, repair, remux, copy, or registry
  rewrite occurs during verification. The selected root must be trusted and immutable;
  this is not protection against concurrent hostile filesystem replacement.
- Metadata is loaded once through `session_contract::load`, which returns a
  `LoadedPackage` only for accepted conformant/degraded metadata. Rejected inputs
  never reach a media subprocess. The existing metadata CLI retains its behavior.
- Processes use argument arrays, explicit input paths, null stdin and no shell.
  `FFREPORT` is removed so inherited configuration cannot create report files.
- Protocol policy is `file` only. Container policy allows mov, matroska/webm, avi,
  wav, flac, mp3, ogg, aac and mpegts; playlist/concat demuxers are excluded.
- ffprobe uses one decoder thread, 10 MB probing, 10-second analysis, a 64 MiB
  **single-allocation** ceiling, 1 MiB header stdout and 64 KiB stderr limits. Packet
  output is streamed with a 4 KiB per-record limit and one summary per known stream,
  not buffered in proportion to recording length. These controls
  are not a complete OS sandbox or a total process-memory bound.
- Default deadlines: 300 seconds per asset, including hashing, and at most 30 seconds
   for each ffprobe invocation, shortened to the asset's remaining budget. The header
   and packet stages share that asset budget and the same cancellation/kill/reap runner.
   Complete scans of long recordings may need larger library `Options` deadlines;
   the current defaults are conservative, not long-session performance evidence.
   Inspection timeout requests
  termination; return can extend until the process has exited and been reaped.
- `Options` exposes the tool path, deadlines and a cancellation token to library
  callers. Cancel the token **and await `verify`** for orderly child cleanup. Merely
  dropping the future retains only a best-effort kill-on-drop fallback.
- File IO can be cancelled separately. Probe timeouts and cancellation are owned by
  the process runner, which explicitly kills and awaits the child. An outer timeout
  must not wrap and discard the entire verifier future.

Run untrusted media tools in a restricted environment. The validation image defaults
to UID/GID 65534; only its synthetic demo/fixture ownership is adjusted during build.
User input permissions are never modified. Network-disabled/read-only usage is shown
below and was exercised successfully.

## Run and reproduce

With current stable Rust and ffprobe installed:

```bash
cargo run --locked -p longcut-session-media --bin session-media-inspect -- --help
cargo run --locked -p longcut-session-media --bin session-media-inspect -- /path/to/package
```

Generate a fresh, explicitly synthetic 10-second package with FFmpeg:

```bash
cargo run --locked -p longcut-session-media --example make_fixture -- /tmp/new-session-demo
```

The output directory must not already exist. The example generates a small H.264/AAC
recording, copies it into reference/clean roles, and writes matching size/hash/stream
declarations. It is a test package, not independently captured broadcast evidence.

In this workspace:

```bash
docker build -t longcut-session-media-check -f crates/session-media/Dockerfile .
docker run --rm --read-only --network none --cap-drop ALL \
  --security-opt no-new-privileges:true \
  longcut-session-media-check /tmp/session-media-demo

# Metadata exists but physical media is intentionally absent: exits 1.
docker run --rm --read-only --network none \
  longcut-session-media-check /src/fixtures/edit-session-packages/complete

# Files/hashes/headers are valid, but one source range overruns by 0.5 ns: exits 1.
docker run --rm --read-only --network none --cap-drop ALL \
  longcut-session-media-check /tmp/session-media-overrun
```

The private Docker daemon is remote, so agent checkout paths cannot be bind-mounted
as though they were daemon-local. Use a disposable container and `docker cp` for other
test inputs, preserving read access for the inspecting user.

## Evidence and remaining work

- 32 media tests cover real FFmpeg-generated files, immutable originals, missing and
  corrupt files, size/hash/header mismatches, unsupported hashes, symlinks, playlist
  rejection, Unicode CLI paths, exit codes, unavailable tools, bounded output,
  timeout cleanup, and coordinated cancellation after the child has actually started.
  Packet-specific cases include negative audio PTS, positively shifted recordings,
  exact/subnanosecond endpoint checks, missing timing, arithmetic overflow, bounded
  streaming records and reaping a timed-out packet scanner. Three extra timebase
  tests protect physical comparisons across clocks without nanosecond conversion.
- Two metadata-loader regressions preserve the existing rejection and media-status
  contract. Metadata schemas are unchanged and their golden checks still pass.
- A failing-first regression reproduced an asset-deadline child leak (`process still
  exists`), then passed with explicit kill/reap ownership. Both inner probe and outer
  asset-budget deadlines now have cleanup tests.
- Whole-workspace formatting, Clippy with denied warnings, server release build,
  and **130 Rust tests** passed: 55 server, 24 contract, 32 media, 19 timebase.
- Manual final-image use succeeded as UID 65534 with a read-only filesystem, no
  network and no capabilities. Both generated files passed; metadata-only fixtures
  correctly degraded with `MEDIA_MISSING`. Help was exercised as well.
  The packet follow-up independently exercised good and overrun packages: the good
  video envelope was `[0,153600)` at `1/15360`, audio `[-1024,480000)` at `1/48000`;
  the overrun fixture failed only its offending TimeMap segment while hashes/headers
  continued to pass. These are synthetic recordings, not real-broadcast acceptance.
- Host Rust LSP remains unavailable; compiler/Clippy results substitute for LSP.
  No browser UI, real multi-hour broadcast, live model or cross-OS acceptance is claimed.

The modules separate file IO/hashing, bounded process execution, typed probe parsing,
stream comparison, orchestration and reporting. New Rust files are below 250 pure LOC.
The library neither changes project persistence nor introduces an editor-facing API.

Next integration work is continuous frame/sample coverage and decode/alignment evidence,
followed by backed-up SourceSession/Asset storage, ordinary-file/package adapters and
atomic import. Separate checksum-manifest files, source reconstruction, and the full
real-broadcast M6/Stage 1 acceptance gates remain unfinished.
