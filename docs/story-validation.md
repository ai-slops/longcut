# Manual Story hierarchy validation

## Delivered behavior

The Story workspace now supports Chapters, Beats, purposes, advisory frame budgets,
frozen reviewed-Moment references and explicit Scene assignment. It is a manual
outline, not an automatic story compiler. Outline order and Scene playback order
remain independent; budget overruns never trim or retime media.

The backend uses schema v9 immutable Story versions and exact references in compact
history/idempotency snapshots. Legacy projects have empty Story version 0 and
unassigned Scenes. Missing historical versions fail rather than resolving to the
latest outline. Split Scenes inherit their Beat; new ordinary/AI Scenes remain
unassigned. Assigned Beats and nonempty Chapters cannot be deleted, and actual
assignment changes to locked Scenes are rejected.

## Reproducible real API acceptance

Run against a local server/gateway with FFmpeg:

```bash
LONGCUT_URL=http://127.0.0.1:3011 node tools/story-acceptance.mjs
```

The driver creates an isolated synthetic project; it does not delete user projects.
It builds reviewed Moments and three Scenes, creates/reorders Chapters and Beats,
pins Moment versions, assigns two Scenes and preserves an unassigned manual cut.
It verifies locked-assignment and deletion guards, historical Story retrieval,
Undo/Redo and real final-render lineage.

Latest observed run:

- Project: `35d41800-14c6-4b63-963f-e70551dcc3a2`.
- Final render: `ef813930-4583-402f-afe6-26380dccf740`, queued at project revision 19.
- Queued Story version: **17**; live Story subsequently advanced to **20**.
- The final lineage retained the old Chapter title `문제 발견`, Beat IDs/titles and
  frozen Moment versions despite later outline edits and Moment archival.
- The third manual Scene stayed unassigned and all Scene ranges/order stayed intact
  during outline reordering.

## Browser verification

Real browser interaction exercised editing, assignment, pinning, temporary
Chapter/Beat lifecycle and Scene Earlier/Later controls. The final lead run verified:

- Beat purpose and target-frame edits save normally without deferred React-event errors.
- `1.5`, `1e3` and `2147483648` budget inputs disable Save rather than throwing.
- Moving a Scene Earlier then Later restores its original order and leaves Story
  unchanged; moving a Chapter leaves every Scene unchanged.
- Pristine/self-saved forms reconcile new versions, while conflicting dirty drafts
  remain visible and require explicit reload.
- A 60-frame Scene assigned to a 45-frame Beat displays **Δ +15**, without cutting it.
- Existing pins remain visibly historical after their Moment head is archived.

Runtime verification found a short-window layout regression: a fixed Scene-panel
minimum height squeezed sibling grid rows to two pixels and intercepted Save clicks.
The final Story workspace uses content-sized rows and a single outer scroll region.
Normal, non-forced Playwright clicks now reach the controls.

Thirteen final viewport captures are stored as
`artifacts/story-final-{outline,beat,links,scenes}-{1280,768,375}.png` plus
`artifacts/story-final-scenes-bottom-375.png`. The mobile Scene pair covers the full
tall panel. Document width equals viewport width at all three sizes. Independent
final functional/visual review passed after the layout correction.

## Build and test evidence

- Final full workspace tests, formatting and all-target Clippy with warnings denied
  passed in the isolated `longcut-story-check` Docker container.
- Locked release build passed; its actual binary served the API/browser tests.
- Web suite: **91 passed**; TypeScript/Vite production build passed.
- TypeScript/CSS LSP remained unavailable. Compiler/build and real browser checks are
  the evidence, not an LSP-clean claim. Upstream Zod annotation notices remain nonfatal.

The execution container restarted during this work and lost temporary servers and
browser dependencies. The runtime was restored and acceptance/screenshots were
regenerated against fresh synthetic data. No old screenshot was relabelled as new.
The host linker was absent after restart, so final Rust verification used Docker.
Build contexts explicitly excluded environment files; nonsecret runtime settings
and evidence did not require reading `*.env`.

## Remaining alpha scope

This completes the manual hierarchy foundation, not automatic semantic story
analysis, outline-to-timeline compilation, inherited Story locks, general media
retiming or full alpha acceptance. Exact/incremental preview, richer scene/audio
composition, technical QC and the real broadcast/creator release gates remain
separate requirements. Synthetic media is not a substitute for those external gates.
