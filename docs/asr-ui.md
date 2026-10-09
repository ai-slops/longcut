# Local ASR UI

The Source and Edit workspaces expose **Local transcription** for the selected
Source. The browser first reads `GET /api/v1/asr/status`; it never installs a
runtime, uploads media to a cloud service, or sends a filesystem path.

## Behavior

- Start is explicit and remains disabled until the server reports the local
  runtime available and the selected Source is ready with audio.
- Korean, English, and automatic language detection map to `ko`, `en`, and
  `auto` in the API contract.
- Queued and running jobs show their literal state and chunk progress. Only
  those states are polled and can be cancelled.
- An unavailable runtime lists missing configuration/files or FFmpeg and offers
  **Check again**. Unavailable means the latest runtime check failed, not that
  local ASR is permanently disabled.
- The current job ID is stored in browser local storage under a key scoped to
  the project and Source. Returning to that Source recovers the job through its
  exact job route because the alpha API has no latest-job endpoint.
- Source changes abort or invalidate in-flight status/job responses, so an old
  Source cannot replace the selected Source's controls or transcript.
- A ready job refreshes the existing Transcript panel. Published transcripts
  are shown as **Local ASR** with engine and requested language metadata;
  subtitle uploads retain `origin: "subtitle_import"` and are shown as subtitle
  files. Neither origin changes the independent Output/program state.

The UI does not auto-start work, retry failed jobs, resume interrupted jobs, or
offer model management. See [the API contract](asr-api.md) for publication and
conflict semantics.
