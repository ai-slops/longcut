# Source transcript import and search API

This is a manually imported subtitle/transcript surface, not automatic ASR.
All cue times are normalized **source editor frames** at the project's frame rate.
Original audio and existing edits/captions are unchanged. Reimport creates a new
immutable version rather than overwriting prior text.

- `POST /api/v1/projects/{project}/sources/{source}/transcripts`
  JSON `{filename: string, text: string}` accepts UTF-8 SRT or WebVTT content.
- `GET /api/v1/projects/{project}/sources/{source}/transcript?query=...&cursor=...`
  returns the newest transcript's first page, or JSON `null` if none exists.
  Use `next_cursor` verbatim for later pages of the same version.

```text
TranscriptPage {
  id: string, source_id: string, filename: string, created_at: string,
  frame_rate: number, cue_count: number, origin: "subtitle_import",
  cues: [{id: string, start_frame: string, end_frame: string, text: string}],
  next_cursor: string | null
}
```

Frames are half-open. Start timestamps round down, end timestamps round up; cues
outside the source fail validation rather than being silently clipped. Limits are
2 MiB input text, 20,000 cues, 16 KiB per cue, 512-byte filename/query and 100 results
per page. Text is displayed as text, never interpreted as HTML or instructions.

Queries of three or more characters use SQLite FTS5 trigram search; shorter Korean
terms use literal substring matching. Only sources in the current project are
accessible. Search/cue selection must seek the same Source viewer and update In/Out
without moving the independent Output playhead.

## Browser workflow

Select a source in Sources or Edit, then use **Import SRT/VTT** in the Transcript
panel. Submit a search to filter cues; **Load more cues** follows the returned
version-pinned cursor. Selecting a cue sets Source seek and In/Out only. Use
**Add scene** to commit that range to the edit. Importing or selecting transcript
text does not itself change the timeline or generated captions.

Changing sources invalidates outstanding requests. Editing the same project keeps
the current transcript search. A failed search keeps the prior successful result
label and reports the error instead of relabelling old cues as new matches.
