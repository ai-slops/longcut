# Curated Moments validation

## Scope

Moments are durable human editorial selections, independent of scenes and automatic
analysis. A positive Source range can be saved without a transcript. Optional cue
or word references identify an exact immutable transcript version; the server
resolves and freezes the quotation. Review is an editorial decision, not verification
of ASR accuracy, timing, synchronization or audiovisual truth.

The lifecycle includes create, revise, review/reject, archive/restore and explicit
reviewed-Moment-to-Scene insertion. Updates clear editorial review. Archived Moments
must be restored before editing/review. Scene references retain the chosen Moment
version even if the current Moment changes. All changes use the existing revision,
idempotency and undo/redo command path.

## Real application evidence

The lead used a native server, production browser UI, generated video and imported
Korean SRT in isolated project `131988e4-29ea-4b46-95d3-4331242c1c12`:

1. Selected cue frames **30–90** and saved `실패 원인 발견` with the `failure` role.
2. Reviewed Moment **v3** and explicitly added it as a two-second Scene.
3. Archived, restored and edited the Moment; the Scene still referenced **v3**.
4. Reimported a different transcript. The Moment retained the original quote
   `실패 원인을 확인했습니다.` and original transcript ID.
5. Fetched historical v3 and verified the original label and evidence.
6. Rendered final revision 7 as `dc4e8fc9-4e80-4276-8ae0-e0f09faf471e`.
   Its lineage contained the original Moment ID, version 3 and immutable-version URL.
7. After upgrading the same database to schema v8, Undo/Redo restored current Moment
   v10 and Scene v3 exactly. Replaying the same command returned the exact cached
   Project. No history was truncated.
8. Set Output to frame **59**, revealed the Moment's Source range **30–90**, returned
   to Edit and verified Output remained **59**.
9. Saved a second manual range without transcript evidence; it remained an independent
   draft Moment with no fabricated quotation.

A second existing QA project used real Whisper word data. The lead selected the
word `마무리되었으며,` at **910–962**, saved and reviewed an audio Moment, and verified
that visual Scene insertion stayed disabled with an explicit explanation.

The UI worker also used real commands while an edit form was open: a newer Moment
version left the unsaved local draft intact, showed a conflict and disabled saving
until explicit close/reopen. Archiving an open editor similarly required restoration.

On the final build, the lead entered `1.5`, `1e3` and an empty frame value in an
evidenced Moment editor. The form remained mounted and Save stayed disabled, without
throwing a BigInt conversion exception. Any Source/range drift clears evidence rather
than silently expanding its scope. The lead also delayed a real project-A command
response, navigated to project B, then released the response: B remained selected.
Historical previews validate project/Moment/version identity and show complete
editorial state and provenance, not just quotation text.

## Persistence and resource boundaries

- Current Project state remains complete. Persisted undo/redo and idempotency results
  use a typed versioned representation containing exact immutable Moment references.
  Only the needed undo/redo target or cached result is hydrated, not the entire history.
- Legacy inline history/results remain readable and are compacted lazily.
- Missing or conflicting immutable snapshots fail closed. Revision overflow is checked;
  it cannot wrap to a previously used Moment version.
- A fixture with **500 maximum-sized heads and 25 edits** measured 520,927 bytes of
  history, 520,882 bytes of cached results, and 10,461,280 bytes of immutable snapshots.
  These are fixture measurements, not a claim that every possible project has that size.
- Historical-version lookup uses the full primary key and only pending references;
  it does not scan all retained versions. A 2,000-unrelated-version regression covers it.
- Schema v8 adds an indexed `transcript_words` projection with transactional bounded
  backfill. Moment word selection queries indexed IDs rather than scanning all word JSON.
- Frozen evidence is capped at 16 KiB UTF-8, with canonical punctuation spacing shared
  with ASR cue assembly. Range validation considers every selected item's extent.

## Regression and compatibility evidence

Workspace tests, formatting, all-target Clippy with warnings denied and native build
passed. Coverage includes legacy Project/Scene JSON, populated word-index migration,
malformed migration rollback, stale/archive precedence, cross-project evidence,
overlapping-cue range bounds, multibyte quote limits, exact historical hydration,
missing-version undo atomicity, nonempty AI proposal publication with compact history,
idempotent replay, imports after Moment creation and frozen render lineage.

Final frontend tests: **81 passed**; TypeScript, lint and production build passed.
Independent functional and CJK/accessibility reviews passed. Final populated,
invalid-input, stale-draft, historical-preview and audio-gating evidence is preserved
under `artifacts/moments/`. Desktop/tablet captures use top/bottom pairs to show
scrollable content; the mobile capture shows the complete page. TypeScript LSP was
unavailable, so the compiler is the type-check evidence, not an LSP-clean claim.
The existing live acceptance driver also passed on schema v8, producing final render
`a46e7f02-814f-42d8-9adb-d64ac3e2eaec` pinned to revision 20 while its isolated project
advanced to revision 21.

This feature does not create automatic semantic Moments, causal relations, Chapters
or Beats. External-model story drafting, general retiming, independently verified
alignment and representative creator/broadcast acceptance remain separate work.
No external LLM was invoked and no `*.env` file was read or searched.
