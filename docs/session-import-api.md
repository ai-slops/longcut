# Session package import API (implementation contract)

The editor accepts a completed package as a directory upload, prepares verification
and editor adapters asynchronously, then commits one undoable source-session import.
Original package data and source identities remain immutable provenance.

All routes are under `/api/v1/projects/{project_id}`:

- `POST /session-imports`: multipart fields named `file`; each filename is the
  package-relative path (strip the browser-selected root directory). Returns 202
  with `SessionImportJob`. No project revision changes during upload/preparation.
- `GET /session-imports`: current jobs, newest first.
- `GET /session-imports/{import_id}`: one job.
- `POST /session-imports/{import_id}/confirm`: JSON `{base_revision, allow_degraded}`;
  returns the current `Project`. Confirmation is one history/revision transaction.
- `POST /session-imports/{import_id}/cancel`: returns the job. Confirmed imports are
  undone through normal undo, not cancellation.
- `GET /source-sessions/{session_id}`: immutable full session record/provenance,
  accessible only while that session is present in the current project snapshot.

```text
SessionImportJob {
  id: string, project_id: string,
  status: "validating" | "prepared" | "publishing" | "confirmed" | "failed" | "cancelled",
  created_at: string, error: string | null,
  session: SourceSessionSummary | null
}

SourceSessionSummary {
  id: string, session_id: string, title: string, duration_ns: string,
  conformance: "conformant" | "degraded" | "rejected",
  imported_at: string,
  assets: SessionAsset[], sources: SessionSource[], warnings: SessionWarning[],
  verification_url: string | null
}
SessionAsset { asset_id: string, uri: string, availability: "online" | "offline" }
SessionSource {
  source_id: string, display_name: string, semantic_role: string,
  kind: "video" | "audio" | "av",
  editor_source_id: string | null, reason: string | null
}
SessionWarning { code: string, path: string, message: string }
```

`Project` gains `source_sessions: SourceSessionSummary[]`, defaulting to empty for
older persisted JSON. Prepared sources are not yet in `Project.sources`; confirmation
adds only verified adapters. Unsupported/offline contract sources remain visible in
the session summary with a reason. A degraded import requires explicit acknowledgement.
Confirmation against a stale revision returns 409 without changing the project.

## Continuous source timestamp normalization

The editor adapter accepts one continuous, unit-rate video segment with at most one
continuous audio segment. A nonzero original media origin is supported when every AV
binding has the same physical origin and that origin is exactly representable at
FFmpeg's microsecond input-offset precision. The mapped video start must also equal
the selected stream's first packet PTS; an interior subrange is rejected because
`-itsoffset` plus `-t` does not prove an exact lossless crop. The adapter applies the
inverse origin offset while remuxing and verifies a zero-origin video/container result.
Duration comes from `media_range.end - media_range.start`, never from an endpoint,
and output frames use an exact checked ceiling so a partial final frame remains
addressable.

The uploaded package and original media asset remain immutable. `SourceSessionRecord`
keeps each original `media_start` and `media_end`; normalization changes only the
generated editor artifact. Each source attempt writes remux and derivative files in
an isolated private directory; only a completely successful artifact set moves into
publishable staging, and every failed attempt removes its owned directory. AAC
priming packet timestamps remain packet evidence and do not redefine the declared
audible source origin.

The adapter still declines discontinuous/split mappings, non-unit-rate or drifted
mappings, unequal AV session intervals, unequal AV media origins, and origins or
durations that would require sub-microsecond rounding. Interior source subranges and
negative origins are not yet claimed as supported interoperability cases. These
sources and their exact original mappings remain in provenance and appear with
`EDITOR_ADAPTER_UNAVAILABLE`; they are not silently approximated.

UI workflow: select directory → upload → poll validating job → inspect all source
statuses/warnings → explicitly confirm (acknowledge degraded if needed) → refresh
project → play/edit eligible sources. Cancellation and failure never partially append
sources. Existing manual upload remains available. No capture application is required.

This document fixes the transport seam for parallel implementation, not a claim that
all routes or arbitrary source timing adapters are already complete.
