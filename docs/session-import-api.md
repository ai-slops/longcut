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

UI workflow: select directory → upload → poll validating job → inspect all source
statuses/warnings → explicitly confirm (acknowledge degraded if needed) → refresh
project → play/edit eligible sources. Cancellation and failure never partially append
sources. Existing manual upload remains available. No capture application is required.

This document fixes the transport seam for parallel implementation, not a claim that
all routes or arbitrary source timing adapters are already complete.
