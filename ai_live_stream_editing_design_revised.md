# AI Collaborative Live-Stream Post-Editing System

## Product, UX, Data, Architecture, and Stage 1 Implementation Specification

Source design: **AI 협업형 라이브 방송 후편집 시스템**, revised completed-session
contract edition, dated **2026-09-01**, supplied by the user.
Repository edition: **2026-09-22**.

**Provenance:** No copy of the supplied document was present in the repository.
This is an explicitly condensed English repository edition, not a verbatim archive
of the Korean original. It preserves the 28-section structure, governing decisions,
scope boundaries, milestone dependencies, and acceptance gates. Detailed original
wireframes, illustrative API/type snippets, and reference descriptions are not
reproduced in full. Requirements below describe the target, not shipped behavior.
For code-backed status, see [implementation status](docs/implementation-status.md).

## 1. Summary and fixed decisions

The product synchronizes four representations of one project: **source**, **story**,
**time**, and **space**. The project graph, not a single timeline file, owns meaning.
The timeline remains a first-class direct-editing interface.

| Area | Governing decision |
| --- | --- |
| Runtime | Local-first Rust core with a React web UI; browser/desktop shell replaceable |
| Editing graph | Session → Moment → Beat → Scene → Layers/Clips → Render |
| AI writes | Typed, reviewable `ChangeSet` operations on stable IDs, not UI automation |
| Human writes | Same command API; manual edits, approvals, and locks survive regeneration |
| Playback | Shared playback/selection context across workspaces, separate source/output positions |
| Composition | Scene owns layout; one Beat may contain several Scenes |
| Feedback | Immutable render time plus semantic, source, transcript, and spatial anchors |
| Media | FFmpeg proxies, exact frames, incremental previews, final render; WebCodecs optional |
| Persistence | SQLite WAL, append-only command journal, checkpoints, content-addressed caches |
| Interchange | Internal graph is authoritative; OTIO/FCPXML are export boundaries |
| Timing | Post-broadcast completed files; quality and reviewability outrank ingest latency |
| Broadcast boundary | Producer-neutral completed `EditSessionPackage`, never a capture-app dependency |

Optimize the loop: find source → draft story → expose uncertainty → review selected
ranges → revise locally while preserving approvals → refine timeline/composition.

## 2. Goals, assumptions, and non-goals

Reduce full-recording review time, preserve explanatory context and state changes,
make every output traceable, allow model replacement without losing human edits,
and keep manual editing usable without AI. Originals and private data stay local
unless explicitly selected for a model request.

Stage 1 assumes 1–6 hour recordings, 1080p30/60 Program reference, approximately two
clean video sources, 2–6 audio roles, Korean-first speech, one local editor, and a
16:9 1080p main output. Limits are configuration/test targets, not implicit coercions.

Not Stage 1: full NLE color/effects/tracking, capture software, concurrent editing,
collaboration SaaS, publishing/billing, lossless NLE round trips, generated speech or
video as default editing material, or comprehensive game/application interpretation.

## 3. Users and principal tasks

- Creator: brief, important moments, narrative intent, style feedback, final approval.
- Editor: source selection, cuts, audio/captions/layout, partial proposal approval, QC.
- Reviewer: frame/range/spatial feedback and immutable-version approval.
- Agent: evidence-backed search, analysis, story/layout proposals, continuity checks.

Support game attempts/strategy changes and development/problem-solving stories.
Also support starting directly with manual source selection and a timeline before
analysis completes; later analysis must propose links rather than replace those cuts.

## 4. Product principles

Keep source/story/time/space synchronized, with the timeline always accessible.
Playback, range selection, feedback, and comparison form one uninterrupted flow.
Keep playhead, hover skimmer, In/Out range, object selection, and keyboard focus
distinct. AI proposes inspectable changes with scope, evidence, duration effect,
uncertainty, protected-edit impact, and preview.

Approvals and property locks outrank regeneration. All writes share typed commands.
Analysis is replaceable and versioned; edit decisions persist. Immediate approximate
composition must be distinguished from exact rendered approval media. Automation is
opt-in by risk/rule, not a global license to rewrite the project.

## 5. Concepts and ownership

`SourceSession` is a completed input recording bundle. `Source` is a semantic role;
`Asset` is a file; `SourceSpan` binds a role to exact stream/channel and time ranges.
`Moment` is an editorial event with evidence and relations. `Story` contains Chapters
and Beats; a Beat owns narrative purpose/budget, while a Scene owns a contiguous
output composition. Layers are spatial objects; Clips are low-level temporal objects.
Layouts provide slots/constraints, and edit motifs combine reusable story/time/space
recipes. Deliverables are independent output variants. RenderVersions are immutable.
Feedback, ChangeSets, and reproducible AnalysisArtifacts remain separate entities.

## 6. Workflow

Import → validate roles/synchronization → brief → analysis/moments → source search →
story → rough cut → scene/audio → feedback → partial revision → QC → export.
This is not a wizard lock: available media is immediately editable; absent analysis
does not block playback or manual assembly. Review cycles connect feedback to intent,
operations, previews, approval, new renders, and explicit remapping decisions.

## 7. Information architecture and common shell

Workspaces: **Brief | Sources | Story | Edit | Review | Deliverables**.
Preserve active deliverable, branch/revision, playheads, In/Out, and object selection.
Use a common top bar, workspace rail, Inspector/Feedback/Agent/History panel, job/error
drawer, and time/quality/cache status. Source, Story, Edit, Scene, Review, and Compare
presets change panel layout, not project data. Background jobs must not lock the UI.

## 8. Required screens

1. Project home: create/open, duplicate/archive, relink, recent renders/review state.
2. Import: file/package discovery, source roles, streams, timing, storage/proxy policy.
3. Sync diagnostics: beginning/middle/end checks, offsets/drift, discontinuities,
   coverage/overlap, missing media, manual correction.
4. Brief: audience, goals/duration, required/excluded content, chronology, privacy,
   copyright, camera/caption/style policies, allowed transformations, output variants.
5. Sources: Program reference/clean source views, search, transcript/observations,
   moments, source usage, context playback, session interval lanes.
6. Story: Chapters/Beats, purpose, evidence, dependencies, budget, alternatives, locks.
7. Edit: output viewer, direct timeline, story/source/transcript context, inspector.
8. Scene: layer tree, composition canvas, crop/fit/safety controls, apply-scope choice.
9. Review: large viewer, frame/range/spatial comments, status, immutable version.
10. Compare: aligned A/B video, semantic/timeline diff, risks, operation-level approval.
11. Deliverables: QC, frozen render revision, subtitles, lineage and export reports.
12. Jobs/errors: progress/stages, retry/cancel, offline media and recovery actions.

Show useful partial states during analysis, retain older previews as visibly stale,
keep project structure available offline, and preserve valid results after AI failure.

## 9. Playback and timeline UX

Always label **SOURCE** versus **OUTPUT**; reserve **Program reference** for the
actual broadcast composite, not the edited output. Preserve each space's last
position. A shared playback context carries target/source/deliverable/version,
playhead, skimmer, range, In/Out, loop, rate, audio mode, playing and follow mode.

Provide Output, Source, Dual, Compare, and up-to-four-source Multiview. Support
frame stepping, J/K/L shuttle, numeric timecode, range/context playback, loops,
None/Page/Center following, pointer-centered zoom, pan, snap and linked selection.
Hover skimming never commits a playhead move. Match Frame reveals contributing
source; Reveal Usage lists every output use rather than choosing one arbitrarily.

Lanes: Chapters, Beats, Scenes, feedback/markers/warnings, role-based video/audio,
optional transcript/observations/health. Source-session lanes remain distinct from
output media tracks. Zoom level controls density: heatmaps → blocks/waveforms →
clips/sentences → frames/words. Canvas with virtualized lanes and interval indexes
must support long sessions without rendering every object into the DOM.

Stage 1 direct operations: append/insert/overwrite, trim/ripple trim, split,
move/reorder, ordinary/ripple/range delete, scene/beat grouping, cuts/dissolves.
Roll/slip are Should; slide optional. Story spine connects Beats/Scenes; positioned
tracks are explicit exceptions. Show lock/ripple effects before commit.

Transcript clicks/selections synchronize time and ranges. Text deletion first shows
cut/breath/context effects. Keep verbatim transcript, edited transcript and captions
separate. Unclassified manual cuts remain valid and survive semantic recompilation.
Accessibility includes keyboard paths, numeric editing, focus rings, non-color
state distinctions, scalable hit targets, and readable object/time descriptions.

## 10. Feedback, review, and versions

Feedback anchors combine immutable RenderVersion/output range/frame with optional
Beat/Scene/Layer/TimelineItem IDs, source spans, transcript tokens, and normalized
content-space pin/rectangle/arrow/path coordinates excluding letterboxing.

Remapping prioritizes stable ID/lineage, source span, utterance/token, semantic
relative position, content fingerprint, then output-time inference. Low-confidence
or missing correspondences enter a review queue, never silently move or disappear.

Distinguish change approval, feedback resolution, and render approval. States:
Open → Acknowledged → In change set → Ready for review → Resolved, with Won't fix
and Superseded branches. Scope may be occurrence/scene/beat/deliverable/project or
an explicitly approved channel-style candidate. Do not generalize local feedback.

Local working-state review may use approximate previews; shared review must use
immutable media. Create versions at review milestones, not every autosave. Compare
by output time or semantic/source alignment; approve independent operations or groups.

## 11. Scene and composition editing

Scene owns active layout, visual layers, captions/graphics policy, default audio
policy and transitions. Support video/image/text/caption/shape/media-overlay and
manual subcomposition layers. Normalized transforms retain anchor, position, size,
scale, rotation, opacity, crop, fit and z-order. Layout templates use semantic slots,
constraints, style tokens, and aspect variants rather than copied absolute pixels.

Provide move/resize/crop/numeric controls, smart guides, safe and hard/soft avoid
regions. Choose current scene versus split-at-playhead application explicitly.
Original composition tracks produce immutable `SessionCompositionHint` evidence,
not editable copies of a capture application's scene graph. Modes: Program
reference, Semantic reconstruction, Template, Custom; offer reference A/B.

Stage 1 prioritizes static 16:9 templates, cuts/dissolves, simple audio role policy,
caption safety and alternatives. Limited endpoint animation is Should, not arbitrary
curves/tracking. Validate off-canvas layers, occluded UI, resolution, offline inputs,
unsafe captions, flicker/layout discontinuity, duplicate reference overlays and locks.

## 12. Agent collaboration and tools

Selected scope is explicit context, not an implied whole-project write permission.
Separate exploration, analysis, proposal, scoped-rule application and feedback work.
Proposal includes plan, actual evidence IDs/ranges, typed operations, expected
effects, affected objects, invalidated previews, risks, assumptions and validation.

Expose narrow paginated read/search tools, ChangeSet drafting, validation and
preview rendering. Only an approved `apply_change_set` mutates project editing state.
Models cannot access DB/files directly or bypass locks. Retrieve summaries → moments
→ utterances/observations → selected frames → exact spans incrementally.

Record producer/model/prompt/tool versions and inputs. Distinguish generated, manual,
imported and template ownership at property level. Default to analysis/proposal;
risky edits always require review. Treat transcript/OCR/metadata as untrusted data,
not instructions. Minimize and audit external-model data transmission.

## 13. Project data model

Separate originals, analysis, intent, timeline projections and renders. Use stable
opaque IDs, monotonic revisions, tombstones, provenance and explicit schema versions.
Time values use integer ticks/rational clocks with string-serialized 64-bit values:
media PTS/timebase, session nanoseconds, output frames/rate; all ranges are `[start,end)`.

Piecewise stream-specific TimeMaps represent offset, drift, split files and
discontinuities. Both ranges must have positive lengths and cannot overlap or reverse
for one asset/stream. Different streams may share session ranges. Missing media is
an explicit health interval, never fabricated mapping. SourceSpan stores both time
spaces, binding and map segment references. Preserve original stream/channel identity.

Analysis artifacts carry input hashes, analyzer/model/config versions, confidence,
payload and supersession. Moments carry spans, transcript/observation evidence,
narrative roles and typed context/causal/repetition/continuity relations. Story
contains Chapters/Beats; Scenes contain Layers; generated timeline items retain
owner/generation key/manual overrides. Unresolvable overrides become conflicts.

Persist compact metadata/relations/commands/FTS in SQLite; media, large analysis,
vectors, tiles, segments and checkpoints in files/content storage. Support external
original references or managed copies, cache GC independent of edits, backed-up
migrations and portable recovery checkpoints.

## 14. Commands, ChangeSets, locks, and history

Commands include stable command ID, project/base revision, actor/origin/timestamp,
payload and idempotency key. Validate schema → permissions/locks → preconditions →
domain constraints → transaction/journal/revision → derived invalidation → events.

Separate semantic commands from derived compilation. Operations span story,
scene/layer/audio, timeline, feedback, policy, approval and property locking.
ChangeSets carry intent/evidence/dependencies and permit atomic partial approval only
when selected dependencies hold. Stale changes require explicit rebase/conflict handling.

Property locks can be hard/warn and inherited. Approval has separate content/order/
duration/composition/caption dimensions. Preserve manual overrides by generation key;
encapsulate complex manual ranges. Group drag commits and full ChangeSet application
as undo transactions. Branches share originals/analysis/cache; same-property merges
conflict. Invalidate only affected derived ranges and retain complete audit lineage.

## 15. Technical architecture

React ↔ HTTP/WebSocket ↔ Rust/Tokio/Axum core ↔ SQLite WAL/file store/job scheduler.
Isolate FFmpeg media workers, analysis workers and agent runtime. The core owns
validation, time maps, commands, search and events, not decoding/model execution.
Desktop shells supply OS integration without replacing the core API.

Probe/proxy/thumbnail/waveform/exact-frame/preview/final jobs use argument arrays,
bounded untrusted inputs, cancellation, tool versions, temporary files and atomic
publication. Default to browser video/range requests and MSE segments; WebCodecs is
optional. Approximate interactive composition is not an approval-quality render.

Segment cache keys include sources/ranges/transforms/audio/captions/profile/renderer.
Render handles include transition/audio context; publish trimmed logical segments.
Final output freezes revision/projection, validates sources, renders, checks result,
and records RenderVersion/LineageMap/checksum. Prioritize playhead and requested
previews over background analysis. Recover interrupted jobs without corrupting edits.

Loopback by default; remote access requires authentication/TLS. Prevent traversal
and automatic network-media access. Record operational timings/cache/errors without
credentials or private source content in telemetry.

## 16. Completed `EditSessionPackage` contract

**This is a SOURCE-session interchange package, not a project snapshot, command
history backup, OBS plugin format, or replay of the broadcaster's internal state.**

Normal conformance requires all six elements:

1. Source registry and completed independent editorial media (at least one source
   beyond Program reference); concrete asset/stream/optional channel bindings.
2. Program reference: actual broadcast video and final audio, one AV file or aligned
   video/audio assets. It is reference/fallback media, not the default edit master.
3. Per-stream TimeMaps to the common `[0,duration)` session clock.
4. Non-overlapping visual-composition intervals covering the declared session.
5. Non-overlapping semantic audio-mix intervals covering the declared session.
6. Media availability/damage intervals covering mapped streams and declared gaps.

Visual contributions identify independent source, primary/secondary/reaction/
background role, optional normalized rect/crop/fit/z-order/opacity. Audio contributions
identify independent stem, audible state, optional gain/pan/role/confidence.
Neither track lists Program reference as an independent contribution. Unknown
intervals are explicit partial coverage, not omitted. Missing optional values mean
unknown, not zero/default. Confidence is `[0,1]`; pan is `[-1,1]`.

Health states: available, missing, truncated, corrupt, decode-error, timestamp-gap,
unknown. Stream-less intervals can describe whole-asset problems. Validate references,
stream kinds/channels, time order/coverage, session bounds, existence/checksums and
sample sync against Program reference. URI access is restricted and rooted safely.

Suggested files: `manifest.json`, `sources.json`, `time-maps.json`,
`visual-composition.json`, `audio-mix.json`, `media-health.json`, `media/`, optional
`checksums.json`. Manifest identifies `edit-session-package`, version `0.1.0`,
`package_state: complete`, session ID, duration, reference source and metadata files.
Source registry contains sources/assets; time maps contain segments; other tracks
contain intervals. Namespaced optional extensions may be ignored; incompatible
meaning changes require a major version. Schema and semantic validation are separate.

Verdicts: **conformant** for valid six-element input, **degraded** for recoverable
missing/damaged elements with explicit capabilities, **rejected** when core references
cannot be resolved. Program-only input is degraded but remains editable.

Out of contract: growing files/live ingest, platform chat/donations/viewer metrics,
capture-app objects/event logs/snapshots, overlay URLs/DOM/scripts, exact effects/DSP,
producer lifecycle/control/transport/recording/recovery mechanisms. Their absence
must not produce importer warnings or capability penalties. Producer owns completion
and package creation; editor owns validation/import. No capture-specific code here.

## 17. Stage 1 scope

Deliver a local single-user alpha: completed package OR ordinary media → source
playback/search/transcript → story/rough cut → manual timeline → scene/audio →
frame/range/spatial review → partially approved AI revisions → 1080p/subtitles.
Manual editing must complete the workflow if AI is unavailable.

Must: persistence/recovery, both import paths, proxies/tiles/relink/time maps,
Korean ASR/search/moments, story/scene hierarchy, direct edit operations, synchronized
selection, layer composition, review/version anchors, ChangeSet locks/partial approval,
exact previews/final render, captions/lineage and technical validation.
Should: diarization/OCR/multiview, roll/slip, scene alternatives, simple animation,
privacy regions, loudness assistance, OTIO and one optional domain plugin.
Could: automatic reaction shots, vertical experiments, LAN review, FCPXML, plugin SDK.

First vertical slice (§17.8): 20–30 minute sample → proxies/waveforms → synchronized
scrub → three output ranges → camera scene → range/pin feedback → camera edit or
ChangeSet → exact range preview → 1–3 minute 1080p output. Stabilize this before
expanding whole-session LLM analysis. Package importer must not block ordinary media.

Alpha requires **all**: two real 3+ hour sessions; both import paths in the same
internal model; creator-finished 10+ minute output using manual and AI edits; smooth
review/revision; partial approval preserving manual trim; crash recovery; video,
subtitles and lineage; preview/final timing/composition/audio agreement; semantic
scene/audio initialization using only the minimum package tracks.

## 18. Milestones and dependencies

| Milestone | Scope | Depends on |
| --- | --- | --- |
| M0 | Playback, 10k timeline, incremental preview, package contract, exact time spikes | none |
| M1 | Persistence/revisions, imports/jobs/proxies, shell and diagnostics | M0 |
| M2 | Sync/time maps, ASR/transcript/FTS, source search and moments | M1 |
| M3 | Story/scene minimum, output timeline/commands/undo, feedback/exact-frame request | M1 |
| M4 | General layers/templates/audio, compiler, incremental exact preview, canvas | M3 |
| M5 | Brief/moments/story, protected ChangeSets, agent tools/proposals and conflicts | M2, M3, M4 |
| M6 | Producer-neutral package schema/importer/fixtures, real data and reconstruction | M1, M2, M4 |
| M7 | QC/full render/subtitles/lineage, version remap, checkpoints/export/stabilization | M4, M5, M6 |

Milestones are acceptance packages, not labels to renumber around existing features.
Some slices can exist while an earlier milestone is still partial. Reassess playback
after M0, compilation after M3, preview engine after M4, AI scope after M5, and
package sufficiency after M6. Original estimate is 34–58 person-weeks, not a promise.

## 19. Performance and completion gates

Test target: 6h, four video including reference, six audio roles, ~100k transcript
tokens, 2k moments/scenes, 10k timeline objects and 1k comments. Targets: visual
selection <100ms; warm output seek median 300ms/p95 800ms; source 250/700ms;
metadata open 3s; commit 100ms; undo 200ms; pan/zoom 60fps without sustained <30fps.
Exact frame target 2s; first changed-preview segment 5s; preview/final cuts same
output frame; audio sync ±20ms typical and ±40ms after 6h drift correction.

Committed edits survive crash; original files never change; cache deletion preserves
edits. Final QC checks duration/streams, black/frozen-frame candidates, clipping,
silence, source availability, caption validity/safety, policy/locks, complete lineage
and checksum. Measure human review time, source search, partial approval/rejection,
manual rework, regression, continuity and policy defects, not automation percentage.

## 20. Test strategy

Domain/time properties → DB/media integration → command/compiler goldens → media
goldens → browser E2E → long-session/performance/recovery → human UX → AI evaluation.
Cover fractional rates, VFR, drift/gaps/restarts, malformed IDs/paths/media, idempotency,
undo/dependencies/locks, manual override regeneration, package conformance/degradation,
frame/audio geometry, IME/selection and normalized annotations.

Use numbered video/tone fixtures, real output probing, independent producer packages,
fault injection at transactional/publication boundaries, and prompt-injection fixtures.
Synthetic acceptance must not stand in for real-broadcast or creator acceptance.

## 21. Risks and responses

Principal risks: browser frame accuracy, approximate/exact mismatch, VFR/drift,
NLE scope growth, lost human edits, misplaced feedback, incorrect package semantics,
long-session scale/storage, private-model transmission and export loss. Counter with
exact frames/proxies, rational maps, bounded scope, property protection, multi-anchor
lineage, independent semantic validation/reference sampling, virtualization/caches,
explicit data disclosure, provenance and export compatibility reports.

## 22. Long-term direction

Improve playback/editing quality first, then richer moment/causal analysis and review.
Add deliverable-specific story/layout variants sharing approved facts and source
evidence; domain plugins supply observations, not core-contract dependencies.
Learn explicit channel rules from approved/rejected changes, not silent uploads.
Possible later work: remote workers/review, collaborative branches, GPU compositor,
constrained generated graphics. Never hide direct editing behind chat or depend on
one model, capture app, platform API, or external NLE as internal truth.

## 23. Decisions still requiring evidence

M0 decides codec/container/MSE/exact-frame and composition-track adequacy from measured
fixtures. M1–M4 evaluate storage defaults, Korean ASR engine, story-spine behavior and
preview-engine sufficiency. Cloud permissions, multi-user protocols, round-trip NLE,
personalized models, vertical-layout automation and generation remain deferred.
UX tests decide terminology, panel proportions, skimming, feedback preroll/loop,
scope prompts, proposal detail, approval complexity and preview-state comprehension.

## 24. Reference provenance

The supplied design cites Premiere text/timeline editing and Match Frame; Frame.io
annotations/ranges; Descript scenes/layers; LAVE and VideoDiff research; Final Cut
skimming/auditions; Resolve Source Tape/Dual Timeline; MLT; GES; FFmpeg; WebCodecs;
and OpenTimelineIO. These are design references, not dependencies or independently
reverified citations in this repository edition. The producer-neutral session contract
is a product requirement, not a claim about any one vendor's API.

## 25. Keyboard and input rules

Ignore global shortcuts in editable controls and during IME composition. Keep command
IDs separate from remappable keys, respect platform modifiers/reserved keys, provide
menus/palette equivalents and visible focus. Default Space play, J/K/L shuttle,
arrows ±1 frame, Shift-arrows ±10, I/O marks, Alt-I/O clear, G timecode, F Match Frame,
Shift-S Source/Output, Shift-D Dual, B split, Shift-B scene split, Delete ordinary
delete, Shift-Delete ripple, C comment, M marker, N snap, Ctrl/Cmd-Z undo.

The supplied source has conflicting shortcuts: §9 gives Shift+/ context playback and
§25 gives looping; §9 assigns Shift+Z whole fit and §25 selection fit. Do not silently
implement both: finalize command IDs/keymap before the corresponding UI increment.
Crop uses Shift-C; annotation keys are review-context-only. Comment submit uses
Ctrl/Cmd-Enter. Pointer-centered zoom and content-space annotation must survive resize.

## 26. API and state contracts

`/api/v1`, consistent snake_case, cursor pagination, string 64-bit times/revisions,
server-issued media handles, generated Rust/TS/tool contracts and runtime validation.
Typed commands are the shared mutation path; dedicated REST workflows must dispatch
through the same validated transactions. Imports inspect first and confirm atomic
session registration only after completed-input validation.

Provide project/history/capabilities, assets/sessions and interval-track windows,
timeline/time-map/usage/frame/preview, search/transcript/moments/analysis, story/scene,
commands/batches/undo/redo, ChangeSet validate/preview/apply/reject/rebase,
render versions/compare/feedback/remap, jobs/logs/cancel/retry, export/report.

Events carry cursor/project/revision/type/time/payload and support reconnect replay.
Playback clocks remain client-local, not per-frame server round trips. Errors include
stable code, message, retryability, details and actions. Agent wrappers are narrower
than REST. Analysis workers handshake capabilities/version, analyze bounded ranges,
cancel, report health and shut down. API, project and plugin versions are separate.

## 27. Repository and package boundaries

Target monorepo: `apps/web`, optional desktop/devtools; Rust domain/timebase/command/
project-store/timeline-compiler/render-plan/media-worker/analysis-protocol/agent-tools/
api-server/session-contract crates; generated api-types, playback, timeline-ui,
scene-canvas and shared components; workers, schemas, fixtures, tools and ADRs.
This is a target decomposition, not a mandate to create empty packages immediately.

Pure domain code must not depend on HTTP/SQLite/FFmpeg types. Keep server/query state,
high-frequency playback, transient drag state and local text drafts distinct.
Generate contracts instead of manually duplicating Rust/TS. A renderer-neutral IR
owns graph/lineage. CI: format/lint, domain/property/schema/generated-code checks,
media goldens, browser E2E, builds and license/security checks; large-media nightly.

## 28. Execution checklist and ordering

Before expansion, establish vocabulary/time/ranges/revisions/overrides, codec/platform
matrix, licensed sample media, first-slice UX, preview labels and external-data policy.
The original unchecked lists are requirements, not evidence of missing or completed
code; current status lives separately to avoid rewriting the target around the build.

Dependency order:

1. Exact time types and command/revision core.
2. Ordinary-file mapping into `SourceSession`.
3. Reference/source playback and virtualized timeline.
4. Manual rough cut and feedback.
5. Scenes/layers/exact range rendering.
6. Parallel: transcript/moments/story and completed-package importer/interval lanes.
7. Protected ChangeSets and AI.
8. Real package validation, full output/QC, recovery and stabilization.

First demo must import 20–30 minutes of reference/game/camera/mic/game audio, assemble
three spans, loop and annotate occlusion, move camera directly or by ChangeSet,
compare, output 1–3 minutes at 1080p with subtitles, trace source frames and recover
after forced exit. All steps must work without external editing tools.

Before alpha, verify immutable originals, WAL integrity/checkpoints/backup/relink,
cache regeneration, worker/render interruption, migration recovery, privacy-safe
diagnostics, keyboard/IME/accessibility, offline/analysis-progress behavior, grounded
IDs, revision/lock/constraint enforcement, partial approval/full undo and model-failure
containment. Alpha is not complete until the real-media gates in §17 also pass.
