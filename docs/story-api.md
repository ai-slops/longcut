# Story outline API

The Story workspace is a manual chapter/beat outline. It does not control playback order: `Project.scenes` remains the only playback and render order.

## Project fields

`GET /api/v1/projects/{project_id}` and every successful command return the full current project. The relevant fields are:

```json
{
  "revision": "14",
  "story": {
    "version": "12",
    "chapters": [
      {
        "id": "3dc5d08e-f369-4538-9c19-2cc2a78d4b4e",
        "title": "Opening",
        "purpose": "Establish the problem",
        "beats": [
          {
            "id": "cc5eb430-ad10-458a-b31c-c3a988063351",
            "title": "Cold open",
            "purpose": "Lead with the result",
            "target_frames": "900",
            "moment_refs": [
              { "moment_id": "moment-id", "version": "7" }
            ]
          }
        ]
      }
    ]
  },
  "scenes": [
    { "id": "scene-id", "beat_id": "cc5eb430-ad10-458a-b31c-c3a988063351" }
  ]
}
```

Legacy projects deserialize as `story: {"version":"0","chapters":[]}`. Legacy scenes deserialize with `beat_id: null`.

IDs created by the server are stable UUIDs. Array order is manual chapter/beat display order. Reordering the outline never creates, deletes, trims, moves, or reorders scenes.

## Command envelope

All mutations use:

`POST /api/v1/projects/{project_id}/commands`

```json
{
  "command_id": "b717652a-6937-4b09-a7ce-0e705da39527",
  "base_revision": "14",
  "command": {
    "type": "story",
    "action": {
      "type": "update_beat",
      "expected_story_version": "12",
      "beat_id": "cc5eb430-ad10-458a-b31c-c3a988063351",
      "title": "Cold open",
      "purpose": "Lead with the result",
      "target_frames": "900"
    }
  }
}
```

`command_id` must be a UUID. Replaying the identical envelope is idempotent and returns the original frozen result. Reusing the ID with different JSON returns `409 conflict`.

`base_revision` must equal the current project revision. Every successful command advances the project revision. Every story action also requires `expected_story_version` to equal the current `Project.story.version`; this guard is checked before payload and immutable-reference lookups.

## Story actions

All examples below are the value of `command.action`.

### Chapters

```json
{"type":"create_chapter","expected_story_version":"0","title":"Opening","purpose":"Setup","before_chapter_id":null}
```

```json
{"type":"update_chapter","expected_story_version":"1","chapter_id":"chapter-id","title":"Opening","purpose":"Revised setup"}
```

```json
{"type":"move_chapter","expected_story_version":"2","chapter_id":"chapter-id","before_chapter_id":"other-chapter-id"}
```

Use `before_chapter_id: null` to move or create at the end. A chapter can be deleted only after all of its beats are deleted:

```json
{"type":"delete_chapter","expected_story_version":"3","chapter_id":"chapter-id"}
```

### Beats

```json
{"type":"create_beat","expected_story_version":"4","chapter_id":"chapter-id","title":"Cold open","purpose":"Hook","target_frames":null,"before_beat_id":null}
```

```json
{"type":"update_beat","expected_story_version":"5","beat_id":"beat-id","title":"Cold open","purpose":"Hook","target_frames":"600"}
```

`target_frames` is advisory. It never trims or otherwise changes scenes. It is either `null` or a decimal string from `"1"` through `"2147483647"`.

Move a beat within or across chapters. `before_beat_id`, when present, must belong to the destination chapter:

```json
{"type":"move_beat","expected_story_version":"6","beat_id":"beat-id","chapter_id":"destination-chapter-id","before_beat_id":null}
```

A beat can be deleted only when no active scene has that `beat_id`:

```json
{"type":"delete_beat","expected_story_version":"7","beat_id":"beat-id"}
```

### Frozen moment links

The supplied list replaces the beat's list exactly and preserves its order:

```json
{
  "type": "set_beat_moment_refs",
  "expected_story_version": "8",
  "beat_id": "beat-id",
  "moment_refs": [
    { "moment_id": "moment-a", "version": "7" },
    { "moment_id": "moment-b", "version": "11" }
  ]
}
```

For each newly added pair, the version must be the current head of a reviewed, non-archived moment, and the exact immutable moment-version record must exist. A `moment_id` cannot occur twice in one beat. The same moment may be used by multiple beats. Existing unchanged pairs remain valid after later moment edits or archival.

### Scene assignment

```json
{"type":"set_scene_beat","expected_story_version":"9","scene_id":"scene-id","beat_id":"beat-id"}
```

Set `beat_id` to `null` to unassign. The beat must exist. An actual assignment change on a locked scene returns `423 lock_conflict`; an identical no-op assignment is accepted. Assignment advances `Project.revision` but does **not** create a new Story version.

Split scenes inherit the source scene's `beat_id`. Trim, move, metadata, camera, and AI updates preserve it. Ordinary manual and proposal/AI scene inserts start with `beat_id: null`.

## Version behavior and immutable reads

Outline actions (all actions except `set_scene_beat`) set `story.version` to the command's next project revision. The version is not computed from the old Story version. Therefore undo, redo, and editing after undo cannot collide with an earlier immutable Story version.

Read a stored snapshot with:

`GET /api/v1/projects/{project_id}/story/versions/{version}`

```json
{
  "project_id": "project-id",
  "version": "12",
  "snapshot": { "version": "12", "chapters": [] }
}
```

The lookup is project-scoped and exact. Version `0` has no synthetic immutable row and normally returns `404`. Missing historical references fail rather than falling forward to the latest Story.

## Limits

| Limit | Value |
|---|---:|
| Chapters | 64 |
| Beats in the whole Story | 512 |
| Beats in one chapter | 128 |
| Moment refs in one beat | 64 |
| Moment refs in the whole Story | 8,192 |
| UTF-8 title bytes | 1–200 |
| UTF-8 purpose bytes | 0–2,000 |
| Serialized Story JSON | 4 MiB |

Counts and byte sizes are hard validation limits. Integer/count arithmetic is checked for overflow.

## Errors

Errors use:

```json
{"error":{"code":"conflict","message":"story version changed"}}
```

| HTTP | Code | Typical cause |
|---:|---|---|
| 404 | `not_found` | Unknown project/chapter/beat/scene/version |
| 409 | `stale_revision` | `base_revision` is not current |
| 409 | `conflict` | Story version changed, deletion guard, immutable conflict, invalid new moment head |
| 422 | `invalid_input` | Bounds, malformed budget, duplicate ref, invalid move |
| 423 | `lock_conflict` | Actual assignment change on a locked scene |

## Render lineage

Render enqueue freezes the full project snapshot. The lineage sidecar uses only that queued snapshot, never the current outline. It adds:

```json
{
  "story_version": "12",
  "story_version_url": "/api/v1/projects/project-id/story/versions/12",
  "scenes": [
    {
      "scene_id": "scene-id",
      "chapter_id": "chapter-id",
      "chapter_title": "Opening",
      "beat_id": "beat-id",
      "beat_title": "Cold open"
    }
  ]
}
```

Later outline edits do not alter queued lineage. A legacy/no-Story render emits explicit `null` for `story_version`, `story_version_url`, and scene chapter/beat fields.

## Persistence contract

Schema version 9 stores immutable Story snapshots in `story_versions(project_id, version, snapshot)`. `projects.state` contains the current full Story. Compact history and idempotency caches contain only `story_version`; exact snapshots are hydrated transactionally. Story snapshots are inserted and conflict-checked before references, project state, history, or command-journal rows commit. History is never truncated by Story operations.
