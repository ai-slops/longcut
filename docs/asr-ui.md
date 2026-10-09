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
- Failed or cancelled jobs expose **Resume transcription** only when the API's
  `resume_eligible` field is true. Older job responses that omit the field are
  parsed as false, so legacy jobs never imply that retained work is reusable.
- Resume is an explicit no-body request to the existing job's `/resume` route.
  It keeps the same job ID and language. The UI shows how many chunks are
  retained locally, while explaining that resume first revalidates inputs and
  local model configuration and continues only when they are unchanged. No work
  is resumed automatically after reload or server restart.
- Resume remains disabled while the runtime is unavailable. **Check again**
  stays available so a corrected local CLI, model, or FFmpeg installation can
  be detected without reloading the page.
- An unavailable runtime lists missing configuration/files or FFmpeg and offers
  **Check again**. Unavailable means the latest runtime check failed, not that
  local ASR is permanently disabled.
- The current job ID is stored in browser local storage under a key scoped to
  the project and Source. Returning to that Source recovers the job through its
  exact job route because the alpha API has no latest-job endpoint.
- Source changes abort or invalidate in-flight status/job responses, so an old
  Source cannot replace the selected Source's controls or transcript.
- Polling pauses while start, cancel, or resume is being submitted. Each action
  owns a request generation, so late poll/action responses cannot replace a
  newer Source or job state; failed submissions release the controls instead
  of leaving them busy.
- A ready job refreshes the existing Transcript panel. Published transcripts
  are shown as **Local ASR** with engine and requested language metadata;
  subtitle uploads retain `origin: "subtitle_import"` and are shown as subtitle
  files. Neither origin changes the independent Output/program state.
- Recovering an already-ready job does not announce that old transcript as a
  newly completed one. The Transcript panel still loads the current immutable
  transcript normally.

Resume reuses only checkpointed, validated model-local work. It does not upload
media, switch models, or reuse a partial published transcript. A `409 conflict`
is shown as an explicit non-start: the Source, model/CLI configuration, policy,
checkpoint set, or current transcript may have changed, or the prior worker may
still be stopping. The failed/cancelled job and its displayed progress remain in
place so the user can correct the local condition and choose **Check again** or
resume again. The UI does not auto-start, auto-resume, or offer model management.
See [the API contract](asr-api.md) for publication and conflict semantics.
