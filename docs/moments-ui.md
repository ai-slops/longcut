# Moments UI

## Purpose

Moments are manually curated, versioned Source ranges. They are managed independently from scene order and may exist without a transcript or on an audio-only Source. Editorial review never claims verified ASR, speech accuracy, source synchronization, or audiovisual truth.

## Source workflow

`RangeControls` exposes **Save moment** beside **Add scene**. Saving opens a compact composer for:

- required label, limited to 200 UTF-8 bytes;
- optional note, limited to 4,000 UTF-8 bytes;
- one closed narrative role;
- the current half-open Source range.

Cue and word selections attach the immutable `transcript_id` and either cue IDs or word IDs. The visible transcript text is preview-only. Commands never submit a quote, origin, or evidence timing.

Selection evidence belongs to one exact Source/range. Selecting another Source, marking In/Out manually, using I/O shortcuts, or revealing a different range clears it and shows an explanation. A manual range remains valid with `evidence: null`; ASR is not required.

## Editing and evidence safety

Moment updates always include `expected_moment_version`. When label, note, or narrative role changes without invalidating the range, the client reconstructs the selection payload from the frozen snapshot:

```json
{"transcript_id":"…","cue_ids":["…"],"word_ids":[]}
```

The client never resubmits the frozen quote or timing. If a Source/range edit no longer contains the frozen evidence, the update sends `evidence: null` to explicitly clear it and warns before saving. It never reuses evidence from another Source.

## Story workspace

The Story workspace contains a separate **Moments** panel below the brief and scene order. It supports search plus `draft`, `reviewed`, `rejected`, and `archived` filters. At most 40 matching heads render initially; **Show 40 more** keeps all results reachable without an unbounded DOM.

Each card exposes explicit actions:

- reveal the Source and In/Out range without moving the Program output playhead;
- edit editorial fields and range;
- review or reject as an editorial decision;
- archive or restore;
- add as a Scene only when reviewed, non-archived, and backed by visual media.

Audio-only Moments can be saved and reviewed but cannot become visual Scenes. Busy actions disable conflicting controls. Global command handling retains the existing stale-revision error path, while every entity mutation pins the displayed Moment version.

## Frozen provenance

An inserted Scene stores only the immutable `{moment_id, version}` link. Story and Inspector surfaces display `Moment vX`; they never resolve the latest Moment head. **Preview frozen vX** calls:

```http
GET /api/v1/projects/{project_id}/moments/{moment_id}/versions/{version}
```

The adjacent JSON link targets that same project-scoped endpoint. Frozen quotes and transcript origins are rendered only from backend responses. Later Moment edits, review changes, archive operations, undo, or branching do not rewrite Scene provenance.

## Compatibility and limits

- Legacy Projects parse with `moments: []`.
- Legacy Scenes parse with `moment: null`.
- Project capacity is 500 current Moment heads, including archived heads; the backend remains authoritative.
- Frozen quotes may be up to 16 KiB and wrap with CJK-safe line breaking.
- Visible copy is at least 12px. Narrow/coarse-pointer actions use 44px targets, wrap instead of clipping, and preserve keyboard focus treatment.
