# Initial Vertical Slice: Validation and Remaining Work

Date: 2026-09-19. This report covers the first integrated increment, not completion
of the full supplied specification or its Stage 1 alpha acceptance criteria.

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

## Not implemented or not yet proven

This list is intentionally explicit so the initial slice cannot be mistaken for
the requested complete editing system:

- Korean ASR, transcript import/edit/search and word-aligned editing.
- Provider-backed LLM agents, semantic moments/relations, automatic story drafts,
  silence/repetition analysis and automatic layout proposals. Current proposals are
  manually authored typed operations, not simulated AI output.
- Full Session/Moment/Chapter/Beat/Scene graph, generated shared API schema, and
  comprehensive migration/snapshot/project-bundle recovery contracts.
- Rational source/session/output time maps, VFR/discontinuity/drift correction and
  multi-source synchronization diagnostics. This increment edits normalized 30/60
  fps source ranges and is not a proof of full time-model correctness.
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
- OBS native capture/logger, capture-package replay, domain/platform plugins.
- External NLE export, remote reviewer authorization and multi-user collaboration.
- Two real 3+ hour broadcasts, a 10+ minute creator-finished output, cross-OS
  acceptance and comprehensive technical/privacy QC.

## Next implementation sequence

1. Establish generated schemas and full rational time/lineage model before expanding
   persisted project semantics.
2. Add source/transcript analysis and source synchronization diagnostics.
3. Add incremental preview jobs and virtualized large-project timelines.
4. Build provider-backed, constrained agent proposals on the validated command path.
5. Add OBS capture-package ingestion and the remaining render/QC/export contracts.
6. Run the supplied specification's real-broadcast alpha acceptance scenarios.

## Repository publication constraint

Local commits are supported. Pushes to the configured origin
`http://dim-gitea:3000/dim-longcut/app.git` fail because `dim-gitea` does not resolve
inside the agent container. No credentials or remote URL were changed to work around
this infrastructure problem. Publishing requires a reachable origin or restored DNS.
