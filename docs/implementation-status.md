# Revised design implementation status

Audit date: **2026-09-22**. Target: the user's **2026-09-01 completed-session
contract revision**, recorded in the explicitly condensed
[repository design edition](../ai_live_stream_editing_design_revised.md).

## Overall finding

### Explicit ASR checkpoint-resume checkpoint

ASR now persists validated chunk checkpoints, revalidates input/runtime fingerprints
and transcript ownership, and supports explicit resume after cancellation or restart.
Real Whisper processing was interrupted with SIGKILL at 1/2 chunks and resumed to
2/2 without rerunning the first chunk. The final browser flow and schema-v6 migration
were also verified. See [ASR resume evidence](asr-resume-validation.md). This resolves
the ASR-specific resumability gap, not all queued media/render/analysis recovery or
the remaining release gates.

### Overlap-aware word timing and declared-sync diagnostics checkpoint

Local ASR now supports bounded two-second context overlap, indexed neighboring-chunk
reconciliation and persisted model-estimated word units. Source word selection is
available without moving Output. Read-only source-session diagnostics expose exact
declared offsets, drift, coverage gaps and overlaps with bounded output and honest
unknown/unverifiable states. Neither feature establishes measured AV alignment or
general playback retiming. Real two-chunk Korean speech and real package diagnostics
were exercised; a four-sample tiny/base comparison remains directional only.
See [timed-ASR and sync evidence](timed-asr-sync-validation.md). This supersedes older
statements below that all word timing, overlap processing or sync diagnostics are absent.

### Local ASR and nonzero-origin integration checkpoint

Local whisper.cpp jobs now support Korean/English/automatic language selection,
bounded sequential audio chunks, progress, cancellation, interrupted-job recovery,
and atomic immutable transcript publication. The UI connects those jobs to source
search and explicitly marks machine-generated text as unverified. Real English and
Korean speech were processed through the server and browser, not just a fake CLI.

Supported continuous package sources can now have a common nonzero AV origin;
the adapter normalizes it while retaining original mapping endpoints. Interior
subranges, unequal AV origins, discontinuities and drift remain unsupported.

A three-hour synthetic audio import/proxy and 360-chunk ASR cancellation check
passed. This does not replace real-broadcast acceptance or Korean accuracy testing.
See [the evidence and explicit limitations](asr-validation.md). These facts supersede
earlier statements below that ASR or all nonzero-origin adaptation are absent.

### Integration checkpoint: 2026-10-03

This checkpoint supersedes the historical implementation and tool-availability
statements below, but does not replace the remaining alpha acceptance gates.

- Completed-package ingestion now stages uploads, validates metadata and media,
  prepares supported editor sources, and requires explicit confirmation before an
  atomic project/history commit. Degraded imports require acknowledgement.
- Immutable source-session records preserve package metadata, original assets and
  editor bindings. Ordinary uploads and synthetic demos use the same persisted
  source-session model, with probe-only provenance explicitly labelled degraded.
- Full decoding and bounded packet-continuity checks are implemented. Normal
  B-frame reordering is handled with a bounded reorder window; this is not measured
  audiovisual alignment or proof of arbitrary media reconstruction.
- Restart recovery, stale-confirm rejection, import undo/redo and frozen-render
  provenance have automated coverage. Supported editing remains restricted to the
  adapter's continuous, unit-rate, zero-origin mappings; unsupported sources retain
  metadata and explicit reasons rather than being silently approximated.
- UTF-8 SRT/VTT import, immutable transcript versions, Korean text search and
  version-pinned pagination are implemented in the backend. This is subtitle
  import, not speech recognition. The browser now supports import, search,
  pagination and cue selection into the independent Source viewer and In/Out.

Fresh execution in this workspace: `cargo test --workspace` and all-target Clippy
with `-D warnings` passed. The web suite passed all 27 tests and its TypeScript/Vite
production build passed (upstream Zod annotation warnings only). Outstanding Rust
formatting differences were corrected, and `cargo fmt --all -- --check` now passes.
Native Rust, rust-analyzer and FFmpeg are now installed; earlier absence statements
describe historical runs only.

The latest native server was personally exercised with `tools/acceptance.mjs` on
an isolated data directory. Ordinary upload, synthetic media, proxy byte ranges,
editing, undo/redo, revision guards, review operations and immutable final export
passed. Project `d3a40738-0e09-4283-9918-93672be4d46d` reached revision 21 while final
render `4eba732c-2ede-4f78-a025-f49128b7bc93` remained pinned to revision 20 and
provided video, SRT, VTT and lineage downloads.

Browser verification exercised package directory upload, degraded acknowledgement,
confirmation, real proxy playback and undo/redo restoring two editable sources.
It exposed and fixed a checkbox event-lifetime crash. Staged/undone original asset
links are disabled, publishing cannot be cancelled from the UI, and late import
responses cannot replace a different project or a newer revision.

The latest native backend and browser also passed Korean SRT import/search/cue
selection and scene insertion: Source moved to frame 63, In/Out became 63–144,
and Output stayed at frame 0. The UI worker verified 100-to-105-cue pagination and
late-response rejection. Lead verification additionally confirmed that Add Scene
preserves the search and a simulated failed search retains the previous results'
correct label. Mobile cue targets measure 44px. An independent final review passed
the current source and fresh captures at 1280, 768 and 375px:
`artifacts/transcript-final-{1280,768,375}.png`.

TypeScript LSP is unavailable (installation previously declined); successful
`tsc --noEmit` is the type-check evidence, not an LSP-clean claim. Markdown has no
configured LSP. No real multi-hour broadcast or creator acceptance is claimed.

**An initial editing vertical slice exists; Stage 1 alpha is not complete.**
There was no copy of the supplied 28-section document or `EditSessionPackage`
contract in the repository before this audit. Existing documentation described the
implemented slice, not the complete product. In particular, references to required
OBS capture/logger implementation contradicted the revised producer-neutral boundary.
Those references are corrected; no OBS work should be scheduled in this editor.

The initial audit was a source inspection plus focused execution of the timebase library.
The follow-up below adds the completed-session metadata boundary and a fresh full Rust check.
Earlier app/browser/render evidence remains dated in [validation.md](validation.md)
and [AI validation](ai-validation.md); it was **not rerun or relabelled as fresh** here.

## Milestones against the original numbering

“Partial” means some constituent behavior exists, not that the milestone's complete
acceptance scenario passed. There is no evidence-based overall percentage.

| Original milestone | Status | Evidence in code | Remaining acceptance work |
| --- | --- | --- | --- |
| M0: technical spikes | Partial | `crates/timebase/` and `crates/session-contract/`; generated package schemas/fixtures and initial media/render path | 3h seek/frame accuracy, 10k Canvas, MSE segments/audio boundaries, real package media validation, codec/OS matrix, randomized time properties |
| M1: core and ingest | Partial | `crates/longcut-server/src/store.rs`, `media_http.rs`, existing proxy/thumbnail/waveform path; standalone package loader | Separate persisted Asset/SourceSession model, stream/channel bindings, migration/checkpoints, relink/storage policy, prioritized jobs, editor package-ingest wiring |
| M2: source exploration, sync, transcript | Prerequisite only | Ordinary source playback; exact mapping library now available independently | No integrated piecewise maps, sync diagnostics, Korean ASR, transcript/FTS, semantic/manual Moment entities or observation lanes |
| M3: output timeline and feedback | Partial vertical slice | `src/domain.rs`, `src/domain/review.rs`, `src/model/project.rs`; existing editor and acceptance driver | Full Story/Chapter/Beat/Scene projection, general clip/role operations, exact-frame service, full interaction/virtualization acceptance |
| M4: scene composition and exact previews | Partial vertical slice | `Camera` geometry in `src/model/project.rs`; FFmpeg render requests in `src/media_http.rs` | General layers/crop/fit/templates/audio, scene compiler, content-addressed incremental range segments, reconstruction/reference comparison |
| M5: story and AI ChangeSets | Partial | `src/domain/review.rs`, `src/ai/validation.rs`, `src/store/ai_publication.rs` | Transcript-grounded Moments/relations/story, operation dependencies, full property lock/approval/override graph and continuity validation; successful real-model editing evidence remains separate |
| M6: completed session contract and real packages | Metadata, file and packet-extent preflight implemented | `crates/session-contract/` and `crates/session-media/` provide metadata validation, file/hash/header checks, exact TimeMap packet envelopes and synthetic fixtures; current editor import remains single-file | Decode/continuous playable coverage/alignment validation, neutral importer/atomic registration, composition/mix/health storage and lanes, hints, real independent-producer fixture |
| M7: QC, export and stabilization | Partial vertical slice | Frozen render enqueue in `src/media_http.rs`; video/SRT/VTT/lineage in initial acceptance | Full QC/audio, semantic feedback remapping, recoverable portable project/checkpoints, packaging/platform and real-session acceptance |

Paths abbreviated `src/…` in the table are under `crates/longcut-server/`.

### Implemented behavior must not be overinterpreted

- The persisted model has `Source`, `Scene`, optional `Camera`, comments, proposals
  and renders. A Story screen is not evidence of the full Moment/Chapter/Beat graph.
- Projects still accept only normalized integer **30 or 60 fps** in `Store::create`.
  A standalone rational library does not make existing imports VFR/drift-correct.
- Whole-scene locks and captured operation preconditions are valuable, but are not
  the specification's complete property approval/inherited lock/manual override model.
- A finished FFmpeg preview is not an incremental segment cache or exact-frame API.
- Scene-local pins and preserved orphan comments are not semantic version remapping.
- Provider-backed scoped proposals are not ASR, silence/repetition detection, or
  automatic understanding of a multi-hour broadcast. See [AI editing](ai-editing.md).

## Work completed in this audit

1. Added the missing design as a labelled condensed repository edition, retaining
   all 28 chapter topics and separating target requirements from current behavior.
2. Corrected capture-app coupling in the README, implementation contract and roadmap.
3. Implemented the first ready prerequisite in §28.13: **exact time and mapping
   primitives**, as the independent `longcut-timebase` workspace library.
4. Reviewed and fixed three new boundary defects before delivery: rounding an offset
   instead of a negative destination timestamp, overflowing an intermediate i64
   duration, and serializing large rational components as unsafe JSON numbers.

The new library has no IO, DB, HTTP, media-worker, UI or model dependency. It provides
typed positive clocks, string i64 timestamps/rates, checked rational arithmetic,
explicit rounding and per-stream piecewise mapping with gaps/overlap/order validation.
Existing persisted projects and server API behavior are unchanged.

### Fresh verification evidence

From the repository root:

```bash
docker build -t longcut-timebase-validation:latest -f crates/timebase/Dockerfile .
docker run --rm longcut-timebase-validation:latest
```

- Locked library build, **16 regression tests**, formatting and all-target Clippy
  (`-D warnings`) passed in the Docker build.
- The three new boundary regressions first failed with `Time(Overflow)`, unsafe
  numeric JSON and `SessionTime(0)` instead of `SessionTime(-1)`, then passed after
  the corresponding fixes.
- The public example was independently executed through the built library:

```text
mapped_session_ns=500500000
inverse_source_ticks=-24000
gap=None
invalid=Err(InvalidMediaRange)
large_exact_ns=9007199254740993
```

Rust/cargo and rust-analyzer are absent on the host. Docker compilation and Clippy
are the diagnostic evidence; **no LSP-clean claim** is made. This turn did not run
the full existing server/UI suite, launch the editing app, call an external model,
or test real broadcasts. The changed runnable surface is the independent library.

## Follow-up: completed-session metadata boundary

Implemented on 2026-09-22: [EditSessionPackage 0.1 metadata schemas and inspector](session-contract.md).
The six generated schemas are published in `schemas/edit-session-package/schema-bundle.json`;
`fixtures/edit-session-packages/` contains complete and Program-only synthetic inputs
plus a 23-case mutation matrix. Validation reuses the exact timebase and distinguishes
metadata `conformant`, `degraded`, and `rejected`, always with media verification
explicitly `not_performed`.

The inspector checks references, kinds/channels, map/track/health coverage and overlap,
version, geometry, namespaces and safe bounded local paths. Review fixes cover short
Program reference, intersecting channel bindings, partial reconstruction overclaims
and metadata amplification. The original editor API and persisted projects are unchanged.

Fresh evidence: 22 contract tests and 16 timebase tests pass; generated-schema equality
and JSON Schema fixture checks pass. The root Docker check also passes server release
build, full-workspace formatting/Clippy and all **93 Rust tests**. Manual inspector
execution verified help and complete/Program-only/missing-directory results. Rust LSP
is still unavailable. Browser, live-model, real-media package and migration/importer
acceptance were not performed. Earlier app acceptance remains historical evidence.

## Follow-up: actual file and stream preflight

Implemented on 2026-09-27: [read-only media preflight](session-media.md), built on an
accepted typed package from the existing metadata gate. It checks actual regular
files, byte count, optional SHA-256 declarations and ffprobe stream indexes/kinds,
codecs, dimensions/audio metadata and declared time bases. It records per-check
results/tool versions and keeps full decode, media-bound and alignment checks
explicitly `not_performed`. No project state or editor API was changed.

Safety includes confined nonsymlink media paths, local standalone-file demuxers,
bounded output, file/probe deadlines and cancellation that awaits child reaping.
A failing regression reproduced the asset-timeout cleanup bug before the fix.
Manual QA also caught synthetic fixture permissions in the validation image; only
that image's generated data ownership was changed, never user media permissions.

Fresh evidence: **112 Rust tests** pass (55 server, 24 contract, 17 media, 16 timebase),
with full formatting, Clippy and server release build. Actual CLI inspection of the
generated 10-second package succeeded in a nonroot/read-only/network-disabled container;
the metadata-only fixture correctly returned degraded/missing-media. No real broadcast,
decode/alignment, migration/importer or browser acceptance is implied.

## Follow-up: exact TimeMap packet envelopes

The media inspector now scans packet timestamps/durations to EOF and compares each
declared source interval against its asset/stream's exact outer packet envelope.
It preserves negative and reordered PTS, compares differing clocks without rounding,
and exposes per-segment and aggregate passed/failed/unverifiable states. Missing
timing, invalid durations, overflow or incomplete scans never count as a pass.
The scan streams bounded records and reuses the proven timeout/cancellation runner.

A failing-first real-file regression demonstrated that a 0.5 ns overrun previously
passed preflight; it is now detected precisely. Positive-offset and negative AAC PTS,
absent TimeMaps, arithmetic extremes and scanner failure/cleanup also have regressions.
Oracle's targeted read found no blocking issue within the explicit extent-only scope.

Fresh evidence: **130 Rust tests** pass (55 server, 24 contract, 32 media, 19 timebase),
plus full formatting, Clippy and server release build. Final CLI QA used nonroot,
read-only, network-disabled containers: valid packet extents passed; a valid-media
package with an overlong TimeMap returned `degraded` / `TIME_MAP_RANGE_OUT_OF_BOUNDS`.
No continuous coverage, decode, AV alignment, editor importer or alpha completion is claimed.

## Next executable work, in dependency order

1. **M0-D deeper media verification:** extend the now-implemented file/hash/header/
   packet-extent preflight with full/range decode, continuous frame/sample coverage
   and sampled alignment evidence. Outer packet extents are not full media conformance.
2. **M1/M2 integration:** introduce Asset/SourceSession/stream/channel/time-map storage
   with an explicit backed-up schema migration. Adapt ordinary-file ingest and the
   package importer into the same model; preserve old projects and command history.
   Add atomic-import, rollback, source/proxy map and source/output lineage fixtures.
3. **M0/M3/M4 UX gaps:** exact frames, incremental range previews, synchronized
   playback and virtualized long timelines. Stabilize the complete manual slice.
4. **M2/M5 analysis:** Korean ASR and transcript search, Moments/relations/Beats, then
   grounded story proposals, operation dependencies and property protection.
5. **M6/M7 acceptance:** independent real packages, audio/QC/export/remapping,
   forced-interruption recovery and the real-broadcast alpha gates.

These follow the revised plan's dependencies rather than treating the presence of
an Agent panel as permission to skip the media/time foundation.

## Unresolved design details and external gates

- Original §9 and §25 conflict on Shift+/ and Shift+Z meanings. Finalize command
  IDs/keymap when implementing those interactions; no shortcut was changed here.
- Package field adapters now have generated snake_case schemas and typed metadata;
  persistence/importer integration must retain their distinction from internal timebase
  wire types, and must not promote metadata-only inspection to verified media conformance.
- TimeMap currently validates pairwise and scans for lookup. Large-track indexing
  and property/performance testing remain necessary before long-session claims.
- Licensed real 3+ hour media from two broadcast types, a creator-finished 10+ minute
  result, independent package production, cross-OS and observed UX acceptance are
  still required. Synthetic tests cannot discharge those gates.

## Stage 1 release checklist

- [ ] Two real 3+ hour sessions import and generate proxies successfully.
- [ ] Ordinary files and completed packages share the internal source-session model.
- [ ] Creator completes 10+ minutes using both manual and AI edits.
- [ ] Playback → range/spatial comment → scene revision → review works continuously.
- [ ] Partial AI approval preserves manual trims and protected properties.
- [ ] Forced-exit recovery and interrupted workers preserve committed data.
- [ ] Video, SRT/VTT and complete source lineage are produced and technically checked.
- [ ] Preview/final cut, layout and audio agreement meets defined tolerance.
- [ ] Package time/composition/mix/health alone initialize reconstruction/audio.

Unchecked gates are not all wholly absent features; they are the complete alpha
conditions for which the required end-to-end evidence has not yet been established.
