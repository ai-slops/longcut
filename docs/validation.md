# Historical Initial Vertical Slice Validation

Date: 2026-09-19. This report covers the first integrated increment, not completion
of the full supplied specification or its Stage 1 alpha acceptance criteria. It is
retained as historical evidence and must not be read as the current feature inventory.

For current delivered behavior and outstanding alpha gates, see
[implementation status](implementation-status.md). Later feature evidence is recorded
in [ASR validation](asr-validation.md), [timed ASR and sync validation](timed-asr-sync-validation.md),
[Moment validation](moments-validation.md), [Story validation](story-validation.md),
and [PART runtime acceptance](part-runtime-acceptance.md).

## Implemented and exercised

- Rust/Axum project service with SQLite WAL, command journal and monotonic revisions.
- Persistent managed media originals, FFprobe imports, proxy, thumbnail and waveform generation.
- Streaming HTTP byte ranges, including suffix/clamped ranges and 416 responses.
- Three-span manual assembly, trim, split, scene reorder/delete, undo and redo.
- Scene title, purpose, caption text, camera overlay, normalized geometry and locks.
- Scene-local range feedback and normalized pins, resolution status and preserved evidence.
- Explicit editor-authored ChangeSets, property preconditions and partial approval.
- Immutable revision-pinned FFmpeg renders, video, SRT, VTT and source lineage.
- React project home and Sources, Story, Edit, Review and Deliverables workspaces.
- Independent Source/Program playheads; program-to-source mapping and bounded playback.
- Approximate browser composition versus exact render review, including independent
  side-by-side version playback. Comparison is not synchronized A/B alignment.
- Same-origin Node gateway, isolated Docker runtime and persistent project volume.

## Automated checks

| Check | Result |
| --- | --- |
| Rust tests | 19 passed |
| Rust formatting and Clippy | Passed, Clippy with warnings denied |
| Locked Rust release build | Passed |
| Frontend TypeScript and Vite production build | Passed |
| Frontend schema/draft regressions | 7 passed |
| Node gateway regressions | 6 passed |
| Biome | Passed; configuration deprecation information remains |
| Docker Compose configuration and runtime | Passed |
| Live API acceptance driver | Passed, including frontend wire-schema parsing |

Rust and TypeScript language-server diagnostics were unavailable in the environment.
Compiler, typecheck, Clippy and Biome checks were used instead; no LSP-clean claim is made.
Vite reports harmless annotation warnings originating in the installed Zod dependency.

The acceptance driver creates an isolated synthetic project and exercises streamed
multipart import, seeking, edits, locks, stale revisions, idempotency, partial
approval and rendering. It verifies that a later edit does not change the queued
render revision. Synthetic inputs are labelled and never represented as broadcasts.

## Hands-on runtime evidence

The lead drove the deployed application with real Chrome 153, without API interception:

1. Opened an API-generated project containing camera ChangeSets. Found and fixed a
   Rust `Option` / Zod nullability mismatch that had prevented project reopening.
2. Confirmed source/proxy video decoding with `readyState=4` and `videoWidth=1280`.
3. Played through three edited scenes and observed automatic stop at output frame
   84, the final valid frame of an 85-frame program.
4. Dragged the camera overlay, saved the geometry, and observed a committed revision.
5. Saved a range comment, locked a scene, verified disabled trim, and undid the lock.
6. Created a two-operation proposal and approved only its camera operation. The
   proposal became partially applied while the manual trim remained unchanged.
7. Requested a final render through the UI, played the actual output at
   `videoWidth=1920`, and initiated an SRT download through the rendered link.
8. Verified same-scene title save/undo/redo updates the visible form correctly.
9. Deleted a commented scene and confirmed preserved feedback cannot seek into an
   unrelated scene; undo restored the scene.
10. Confirmed a short program's seek maximum is 84, and a keyboard frame step moves
    from 0 to 1. Selecting the same comparison version yields one monitor, not an
    empty comparison column.

FFprobe on the API acceptance output reported H.264, 1920x1080, 85 video frames,
AAC at 48 kHz, and 2.856 seconds of container duration. The video frame span is
85/30 seconds; the container duration includes AAC packet padding. Sample-accurate
audio boundary equivalence has not been established.

The initially available open-source headless-shell browser could not decode H.264.
It was replaced by actual Google Chrome for media QA. Download testing required
enabling downloads in the CDP test context; this was a test-harness setting, not an
application code workaround.

## Persistence and recovery

The deployed core was restarted normally and then terminated using SIGKILL and
started again. Project revision 28, its three scenes, two comments and two ready
render records were recovered from the persistent volume. Later browser-only undo
checks advanced the test project's revision normally.

Unit regressions additionally cover preservation of operational source/render state
across undo/redo and interrupted-job recovery. Killing a worker at every filesystem
publication boundary, migration rollback and long-session corruption testing remain
future acceptance work.

## Visual QA

Fresh real-backend captures cover all six pages at 1280, 768 and 375 pixel widths:
`artifacts/final-{home,sources,story,edit,review,deliverables}-{1280,768,375}.png`.
All measured document widths match their viewports. Media artboards retain a 16:9
coordinate space. Header and workspace bounds no longer overlap on mobile.

Independent visual review passed. Independent functional review initially requested
repairs to timeline bounds, draft synchronization, orphan feedback, comparison state
and control sizes. Those repairs were made, checked in the live browser, recaptured,
and a fresh independent reviewer returned PASS with no blocking findings.

No pixel-identical reference was supplied, so no pixel-diff fidelity score is claimed.
CJK-specific visual acceptance and assistive-technology walkthroughs remain unverified.

## Gaps recorded at this checkpoint

This list records the 2026-09-19 checkpoint. Later work delivered local ASR and
transcript search, manual Moments/Chapters/Beats, completed-session ingestion, and
declared-sync diagnostics. The current status document above supersedes those items;
the remaining limitations below should not be generalized beyond this dated report.

- Automatic semantic moments/relations, automatic story drafts and
  silence/repetition analysis were not delivered; later provider-backed proposals
  and manual story entities do not imply those automatic capabilities.
- The complete target Session/Moment/Chapter/Beat/Scene graph and comprehensive
  migration/snapshot/project-bundle recovery contracts were not established.
- General piecewise source/session/output mapping, VFR/discontinuity/drift correction
  and measured multi-source synchronization were not established. Later exact timebase
  and declared-sync diagnostics do not prove full audiovisual alignment.
- General multilayer/crop/rotation/fit editing, dissolve transitions, audio stem
  mixing, arbitrary graphics and burned-in caption styling.
- Exact-frame service, content-addressed incremental segment caching, MSE playback,
  render cancellation/retry and a prioritized resource-aware job scheduler.
- Render-to-render semantic feedback remapping. Removed-scene evidence is preserved;
  immutable-render links remain available, but automatic remapping is not claimed.
- Property-level approval/lock UI, linked-group ripple/roll/slip operations,
  manual subcompositions and branching/merge.
- Canvas/interval-index timeline virtualization and measured 6-hour/10,000-object
  performance. The current small-project timeline uses DOM elements.
- Complete composition/mix/health-driven reconstruction remained unproven. Capture
  software and producer-state replay are outside the revised editor scope.
- External NLE export, remote reviewer authorization and multi-user collaboration.
- Two real 3+ hour broadcasts, a 10+ minute creator-finished output, cross-OS
  acceptance and comprehensive technical/privacy QC.

## Historical follow-up sequence

The original sequence from this checkpoint has been superseded by delivered ASR,
source-session ingestion, manual story entities, synchronization diagnostics, and the
shared agent runtime. Use the dependency-ordered work and Stage 1 release checklist in
[implementation status](implementation-status.md) for current planning.

## Repository connectivity and host-assisted recovery

The initial publication failure was caused by the nested agent missing the
controller-approved `dim-gitea` host alias. Commit `fa2e998` forwards that alias
from the read-only DIM Project manifest through a temporary Compose override.
Five regression tests cover the manifest boundary and the actual Compose merge.
Neither the Git remote nor its credentials were changed, and no addresses were
guessed or discovered by scanning other networks.

An attempted workspace restart interrupted the agent and required host recovery.
The host subsequently published `3ed0a4d`, restoring the rootless DinD uidmap helper
ownership and setuid permissions. On resumption, `git pull --ff-only` confirmed
that commit was integrated and all earlier implementation commits were already on
the remote. The approved Git hostname now resolves normally.

The restored application volume had a separate ownership mismatch: the core runs
as UID 10001/GID 999, but `/data`, its artifacts directory and SQLite database were
owned by UID/GID 0. The mount was read-write; the unprivileged process could not
create SQLite WAL files and repeatedly exited with readonly error 1544.

Recovery stopped only the core service, not the DIM workspace or Docker daemon.
The complete 42 MB project volume was archived to the separate Docker volume
`longcut-recovery-20260919-2031`, file `/backup/projects.tar`. Its SHA-256 is
`16b314eeb3fb8a6a6367c04ccc05045f000bf736fa0f8376b270a40c3f76ad98`.
Ownership was then restored to the image's `longcut:longcut` account without
following symlinks or making files world-writable. The SQLite file checksum was
unchanged by that metadata-only repair.

After starting only the core service, validation confirmed:

- Database, FFmpeg and FFprobe health all report healthy.
- All three pre-existing projects remain available.
- The reviewed project remains at revision 34 with three scenes, two comments and
  two ready renders.
- Existing rendered media serves a valid HTTP 206 byte range.
- A separate verification project can be created, edited and reopened successfully.
- The DIM development URL points to the persistent startup gateway on port 3000;
  its UI returns HTTP 200 and its health endpoint reports `ok`.

The recovery backup is intentionally retained. Treat workspace restart as an
infrastructure operation that can disconnect the agent, not routine application
maintenance. Prefer service-scoped actions and coordinate workspace-level recovery
with the host operator.
