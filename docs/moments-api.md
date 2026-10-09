# Moments API

Moments are manually curated, versioned source ranges. They are independent from scenes and transcripts: a Moment can be created before transcript import and can point to an audio-only source. Transcript evidence is optional and never implies speech or audiovisual accuracy.

All mutations use the existing command endpoint:

```http
POST /api/v1/projects/{project_id}/commands
Content-Type: application/json
```

```json
{
  "command_id": "4cc5db94-a108-49a1-a9b5-a85c41511677",
  "base_revision": "12",
  "command": { "type": "..." }
}
```

`command_id` is an idempotency UUID. An identical retry returns the original Project; reusing it with different JSON returns `409 conflict`. `base_revision` must equal `Project.revision`, or the server returns `409 stale_revision`. Entity mutations also require `expected_moment_version`; stale entity versions return `409 conflict`.

## Command DTOs

The closed narrative role set is `context`, `setup`, `goal`, `attempt`, `failure`, `insight`, `payoff`, and `other`. The review status set is `draft`, `reviewed`, and `rejected`.

Create, with no transcript:

```json
{
  "type": "create_moment",
  "source_id": "source-1",
  "start_frame": "1800",
  "end_frame": "2700",
  "label": "The recovery starts",
  "note": "Keep the hesitation before the answer.",
  "narrative_role": "insight",
  "evidence": null
}
```

Create or update with cue evidence:

```json
{
  "type": "create_moment",
  "source_id": "source-1",
  "start_frame": "1800",
  "end_frame": "2700",
  "label": "The recovery starts",
  "note": "",
  "narrative_role": "insight",
  "evidence": {
    "transcript_id": "6e163b80-d5e5-4dbb-971f-50335ef9ade5",
    "cue_ids": ["cue-18", "cue-19"],
    "word_ids": []
  }
}
```

For word-level evidence, send `cue_ids: []` and one or more `word_ids`. Cue and word IDs cannot be mixed. Clients cannot send a quote, origin, or evidence timing. Unknown fields are rejected. The server resolves IDs through the normalized word index against the exact immutable transcript ID, confirms project/source/range ownership, and freezes the verbatim words in canonical ASR order. Canonical spacing omits a space before `. , ! ? : ; ) ] }`; for example, `hello` plus `.` freezes as `hello.`, and Korean words follow the same punctuation rule. A later transcript import does not retarget evidence.

Update uses complete-snapshot semantics for evidence. `evidence: null` clears all frozen evidence. To retain evidence, resend the original immutable `transcript_id` and cue or word IDs; the server resolves and validates them again against the updated source and range.

```json
{
  "type": "update_moment",
  "moment_id": "6426c129-9d5b-4998-9611-12d747659629",
  "expected_moment_version": "13",
  "source_id": "source-1",
  "start_frame": "1830",
  "end_frame": "2730",
  "label": "Recovery begins",
  "note": "Updated note",
  "narrative_role": "insight",
  "evidence": null
}
```

Any update, including an evidence change, resets `review_status` to `draft`.
The target Moment, expected version, and non-archived state are checked before new fields or evidence are resolved. A stale target returns `409` even when replacement fields or evidence are invalid. Archived Moments must be restored before update or review.

Review or reject:

```json
{
  "type": "review_moment",
  "moment_id": "6426c129-9d5b-4998-9611-12d747659629",
  "expected_moment_version": "14",
  "review_status": "reviewed"
}
```

Review is an editorial decision only. It does not verify ASR, speech accuracy, source synchronization, or audiovisual truth. No AI path automatically reviews Moments.

Archive and restore:

```json
{"type":"archive_moment","moment_id":"6426c129-9d5b-4998-9611-12d747659629","expected_moment_version":"15"}
```

```json
{"type":"restore_moment","moment_id":"6426c129-9d5b-4998-9611-12d747659629","expected_moment_version":"16"}
```

Insert a scene explicitly:

```json
{
  "type": "insert_scene_from_moment",
  "moment_id": "6426c129-9d5b-4998-9611-12d747659629",
  "expected_moment_version": "15",
  "title": "Recovery begins"
}
```

Only a `reviewed`, non-archived Moment can be inserted. The source must contain video; an audio-only Moment remains valid but cannot become a visual scene. The new Scene freezes:

```json
"moment": {"moment_id":"6426c129-9d5b-4998-9611-12d747659629","version":"15"}
```

Later Moment updates, rejection, archive, undo, or branching never rewrite that Scene reference. Frozen render lineage includes `moment_id`, `moment_version`, and `moment_version_url` for each linked scene.

## Project and immutable version responses

Every successful command returns the complete Project. Legacy Projects deserialize with `moments: []`, and legacy Scenes deserialize with `moment: null`.

```json
{
  "id": "project-1",
  "revision": "15",
  "moments": [{
    "id": "6426c129-9d5b-4998-9611-12d747659629",
    "version": "15",
    "source_id": "source-1",
    "start_frame": "1830",
    "end_frame": "2730",
    "label": "Recovery begins",
    "note": "Updated note",
    "narrative_role": "insight",
    "origin": "manual",
    "review_status": "reviewed",
    "archived": false,
    "evidence": [{
      "transcript_id": "6e163b80-d5e5-4dbb-971f-50335ef9ade5",
      "cue_ids": ["cue-18", "cue-19"],
      "word_ids": [],
      "quote": "Frozen verbatim transcript text",
      "origin": "subtitle_import",
      "start_frame": "1830",
      "end_frame": "2700",
      "timing_basis": "cues"
    }],
    "provenance": {
      "created_command_id": "4cc5db94-a108-49a1-a9b5-a85c41511677",
      "updated_command_id": "fb9a0742-7e75-4bbc-a610-4317ff221bf4",
      "created_revision": "13",
      "updated_revision": "15",
      "created_at": "2026-10-09T13:00:00Z",
      "updated_at": "2026-10-09T13:05:00Z"
    }
  }]
}
```

Historical snapshots are read-only and remain available after undo/redo and redo-branch pruning:

```http
GET /api/v1/projects/{project_id}/moments/{moment_id}/versions/{version}
```

```json
{
  "project_id": "project-1",
  "moment_id": "6426c129-9d5b-4998-9611-12d747659629",
  "version": "15",
  "snapshot": { "id": "6426c129-9d5b-4998-9611-12d747659629", "version": "15" }
}
```

The real `snapshot` is the complete Moment object shown above. Reads are project-scoped and return `404 not_found` for a mismatched project, Moment, or version.

## Frontend wiring example

```ts
const commandId = crypto.randomUUID();
const response = await fetch(`/api/v1/projects/${project.id}/commands`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({
    command_id: commandId,
    base_revision: project.revision,
    command: {
      type: "create_moment",
      source_id: selection.sourceId,
      start_frame: selection.startFrame,
      end_frame: selection.endFrame,
      label: draft.label,
      note: draft.note,
      narrative_role: draft.narrativeRole,
      evidence: transcriptSelection
        ? {
            transcript_id: transcriptSelection.transcriptId,
            cue_ids: transcriptSelection.cueIds,
            word_ids: transcriptSelection.wordIds,
          }
        : null,
    },
  }),
});
```

Keep the same `command_id` only when retrying the exact same serialized request. Replace local Project state with the successful response. Render `project.moments` independently from `project.scenes`; use `scene.moment` only as frozen lineage metadata.

## Validation and quality limits

- Frame values are decimal strings and ranges are half-open `[start_frame, end_frame)`.
- Ranges must be non-empty and inside the selected source duration.
- Maximum current Moment heads per Project: 500, including archived heads.
- `label`: required, at most 200 UTF-8 bytes. `note`: at most 4,000 UTF-8 bytes.
- One evidence selection contains 1–500 unique cue IDs or word IDs.
- Evidence IDs must belong to the exact transcript, project, and source and fit entirely inside the Moment range.
- Evidence extent is the minimum selected start through the maximum selected end, including overlapping cues or words.
- The server-derived frozen quote is limited to 16 KiB of UTF-8 bytes, including inserted separators. Oversized selections are rejected atomically.
- `Project.revision` supplies globally monotonic Moment versions for command edits; undo/redo also advance the revision, so a branch never reuses an old version.
- Immutable version rows are stored by `(project_id, moment_id, version)` and duplicate versions fail rather than overwrite. Moment versions were introduced in v7; the latest project database schema is v8.
