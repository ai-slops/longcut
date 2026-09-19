# Longcut implementation contract

## Delivery sequence

The supplied specification is the product roadmap. Implementation starts with its
section 17.8 vertical slice, before analysis or automatic story generation. A working
slice is not equivalent to Stage 1 alpha. Stage 1 additionally requires long-session,
multi-platform, Korean ASR, OBS, analysis, and creator acceptance evidence.

## Environment

The initial repository contains a Node 24 server, started by `.dim/agent/start.sh`.
The agent has a private rootless Docker daemon, not the host Docker daemon.
Build the Rust/FFmpeg service in Docker; preserve the Node entry point as a streaming
same-origin gateway. Keep originals and SQLite on a persistent Docker volume.
Do not bind agent workspace paths into the remote daemon: they are different filesystems.
Use a Docker build context or `docker cp` for transfers.

The referenced GitLab DIM repository could not be read anonymously: the raw README
redirected to sign-in, the project API returned 404, and Git authentication failed.
Local `.dim/` files and `dim-development-service --help` are the verified environment
contract until access is provided. The installed proxy README also identifies the
official upstream at <https://github.com/slop-lab/dev-infra-manager>; its runtime and
trust-boundary documentation confirms nested Docker isolation and persistent engine
state. Use `dim-development-service expose --name longcut --port 3000 --ingress
https-ts --require-scheme https` to expose the agent-local gateway. Never persist
credentials in documentation.

## API v1, first increment

All paths below are prefixed by `/api/v1`. JSON uses snake_case. IDs are opaque UUIDs.
Frames and revisions are decimal strings. Projects have an explicit integer frame
rate of 30 or 60; source metadata retains the original time base. All ranges are
half-open. Source selection uses normalized proxy frames at the project frame rate,
with original PTS retained for rendering. Do not label this full VFR support until
time-map acceptance fixtures pass.

### Snapshot

`GET /projects` returns `ProjectSummary[]` with `id`, `title`, `revision`, `updated_at`.
`POST /projects` accepts `{title, frame_rate: 30|60}` and returns `Project`.
`GET /projects/{id}` returns `Project`:

```text
Project {
  id, title, revision, frame_rate, updated_at,
  brief: string,
  sources: Source[], scenes: Scene[], feedback: Feedback[],
  change_sets: ChangeSet[], renders: Render[],
  can_undo: boolean, can_redo: boolean
}
Source {
  id, name, role: "program"|"primary"|"camera"|"audio",
  duration_frames: string, width: number, height: number, has_audio: boolean,
  media_url: string, proxy_url: string|null, thumbnail_url: string|null,
  waveform: number[], status: "processing"|"ready"|"error", error: string|null
}
Scene {
  id, title, purpose: string, source_id, in_frame: string, out_frame: string,
  camera: Camera|null, caption: string, locked: boolean, manual: boolean
}
Camera {
  source_id, in_frame: string, x: number, y: number,
  width: number, height: number, opacity: number
}
Feedback {
  id, body, scene_id, start_frame: string, end_frame: string,
  revision: string, render_id: string|null,
  pin: {x: number, y: number}|null, resolved: boolean
}
ChangeSet {
  id, title, intent, base_revision: string, operations: Operation[],
  status: "ready"|"partially-applied"|"applied"|"rejected",
  applied_operation_ids: string[], evidence: string[]
}
Operation { id, scene_id, type: "set_camera"|"trim_scene", camera?: Camera|null,
            in_frame?: string, out_frame?: string, rationale: string }
Render {
  id, revision: string, kind: "preview"|"final", status: "queued"|"running"|"ready"|"error",
  progress: number, media_url: string|null, srt_url: string|null,
  vtt_url: string|null, lineage_url: string|null, error: string|null
}
```

### Commands

`POST /projects/{id}/commands` accepts:

```json
{"command_id":"UUID","base_revision":"0","command":{"type":"set_brief","brief":"Keep the full final attempt."}}
```

It returns the entire updated Project. `409` means stale revision; `423` means lock
conflict; `422` means invalid input. Errors are `{error:{code,message}}`.
Command types and fields:

| type | payload fields |
| --- | --- |
| set_brief | brief |
| rename_project | title |
| set_source_role | source_id, role |
| insert_scene | source_id, in_frame, out_frame, title |
| trim_scene | scene_id, in_frame, out_frame |
| split_scene | scene_id, offset_frame (scene-local) |
| move_scene | scene_id, before_scene_id (null appends) |
| delete_scene | scene_id |
| update_scene | scene_id, title, purpose, caption |
| set_camera | scene_id, camera (null removes) |
| lock_scene | scene_id, locked |
| create_feedback | scene_id, start_frame, end_frame, body, pin, render_id |
| resolve_feedback | feedback_id, resolved |
| create_change_set | title, intent, operations, evidence |
| apply_change_set | change_set_id, operation_ids |
| reject_change_set | change_set_id |
| undo | none |
| redo | none |

The transaction validates revision, IDs, ranges, locks and complete operation set
before writing state, journal and new revision. Duplicate command IDs return the
original result only if the payload matches. Undo/redo are new revisions, never a
revision decrement. Applying a ChangeSet is one undo step. UI drafts must not mutate
server snapshots. Direct edits mark scenes manual; agent proposals cannot bypass locks.

Feedback offsets are scene-local. Splitting/deleting a scene preserves original
comment evidence; do not silently attach comments to unrelated scenes.

### Media

`POST /projects/{id}/imports?role=program` accepts a streamed multipart `file`, probes
it, registers the immutable original through the core transaction, and returns the
Project. Derivatives update operational status without changing the edit revision.
`POST /projects/{id}/demo` generates original test-pattern media locally, imports it
and returns Project. Demo media must be visibly labelled synthetic, never a real broadcast.
`POST /projects/{id}/renders` accepts `{base_revision, kind:"preview"|"final"}` and
returns Render. Poll `GET /projects/{id}` while jobs run. Render inputs are frozen at
request time. Both kinds use the same compositor. Final output is 1920x1080 H.264/AAC.
Caption text yields SRT/VTT and lineage includes each output scene and original range.
`GET /media/{artifact}` supports HTTP Range; all artifact paths are server-issued.
`GET /health` returns service/tool availability.

Files publish via temporary output and atomic rename. FFmpeg is invoked with argument
arrays, never shell strings. Interrupted jobs become actionable errors on restart.
Media failures never delete semantic state. No arbitrary URL/path imports in this increment.

## UI contract

React provides project home and Sources, Story, Edit, Review and Deliverables workspaces.
Brief is an editable panel. All workspaces share program playhead and selection; source
playhead is separate. Skimming must not overwrite the playhead. Source/program labels
are explicit. Comments and inspector share selected scene. Keyboard shortcuts ignore
text inputs and IME composition. Show approximate preview versus revision-pinned exact
render explicitly. Never show fake AI generation or fake progress.

The initial agent surface accepts explicit typed ChangeSet proposals; provider-backed
story/ASR integrations are separately tracked and never simulated as completed AI work.

## Acceptance evidence

Generate timestamped synthetic video/audio; create and reopen a project; import media;
select three source spans; trim/split/reorder and undo; overlay a second camera source;
write a range/pin comment; apply one proposal operation while preserving other edits;
reject stale commands and locked edits; render a real file; inspect resolution, duration,
audio and lineage; restart the backend and reopen committed state. Drive the browser
through the same flow and record limitations separately from passing checks.
