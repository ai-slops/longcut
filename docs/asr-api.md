# Local ASR API (alpha)

Longcut can run a locally installed whisper.cpp CLI against an existing managed
source. The browser never supplies a filesystem path. The server extracts bounded
16 kHz mono PCM chunks with FFmpeg, invokes whisper.cpp directly with an argument
array, and publishes one immutable transcript only after every chunk succeeds.

This surface is local-only. It does not download models, contact a network service,
or revise the edit timeline/project revision.

## Server configuration

| Environment variable | Required | Default | Constraint |
| --- | --- | --- | --- |
| `LONGCUT_WHISPER_CLI` | yes | none | Path to an executable whisper.cpp CLI |
| `LONGCUT_WHISPER_MODEL` | yes | none | Path to an existing regular model file |
| `LONGCUT_ASR_CHUNK_SECONDS` | no | `600` | Integer `30..=3600`; invalid values use the default |
| `LONGCUT_ASR_MAX_CONCURRENCY` | no | `1` | Integer `1..=4`; invalid values use the default |

FFmpeg remains the server's local `ffmpeg` executable. A job is unavailable unless
the configured CLI and model exist and FFmpeg answers `-version`. Configuration is
read only by the server; there is no API for setting or uploading these paths.

## Runtime status

`GET /api/v1/asr/status`

Always returns `200`:

```ts
type AsrRuntimeStatus = {
  configured: boolean;
  available: boolean;
  cli_available: boolean;
  model_available: boolean;
  ffmpeg_available: boolean;
  missing: Array<"LONGCUT_WHISPER_CLI" | "LONGCUT_WHISPER_MODEL">;
  engine: "whisper.cpp";
  model: string | null; // configured local model path
  default_language: "ko";
  supported_languages: ["ko", "en", "auto"];
  chunk_seconds: number;
  max_concurrency: number;
};
```

`configured` means both required environment variables are present. `available`
also requires the referenced files and FFmpeg to be available. The frontend should
disable Start while `available` is false and may render the individual checks.

## Start a source job

`POST /api/v1/projects/{project_id}/sources/{source_id}/asr`

```json
{"language":"ko"}
```

`language` is optional and defaults to `ko`. The only accepted values are `ko`,
`en`, and `auto`; unknown fields and other language values return `422 invalid_input`.
The source must exist in the project, be `ready`, contain audio, and resolve through
its server-managed `/api/v1/media/{artifact}` URL. Filesystem paths are never
accepted. A source can have only one `queued` or `running` ASR job.

Success returns `202 Accepted` with an `AsrJob`. Poll only while its status is
`queued` or `running`.

## Poll and cancel

- `GET /api/v1/projects/{project_id}/sources/{source_id}/asr/{job_id}`
- `POST /api/v1/projects/{project_id}/sources/{source_id}/asr/{job_id}/cancel`

Both return:

```ts
type AsrJob = {
  id: string;
  project_id: string;
  source_id: string;
  status: "queued" | "running" | "ready" | "failed" | "cancelled";
  progress: {
    completed_chunks: number;
    total_chunks: number; // zero while queued
  };
  language: "ko" | "en" | "auto";
  engine: "whisper.cpp";
  model: string;
  created_at: string; // RFC 3339
  updated_at: string; // RFC 3339
  transcript_id: string | null;
  error: string | null;
};
```

Cancellation is idempotent for terminal jobs. For an active job the server first
persists `cancelled`, then signals the worker. An active FFmpeg or whisper.cpp child
is killed and reaped. No transcript is published. A server restart converts every
persisted `queued` or `running` job to `failed`; interrupted work is not silently
resumed.

## Published transcript

When a job becomes `ready`, `transcript_id` identifies the new immutable transcript.
Read/search it through the existing endpoint:

`GET /api/v1/projects/{project_id}/sources/{source_id}/transcript?query=...&cursor=...`

The response remains `TranscriptPage` from [transcript-api.md](transcript-api.md).
For local ASR it adds:

```ts
{
  origin: "local_asr",
  asr: {
    engine: "whisper.cpp",
    model: string,
    requested_language: "ko" | "en" | "auto",
    result_language: string | null
  }
}
```

Manual imports remain `origin: "subtitle_import"` and omit `asr`. Cues use the same
half-open source-frame coordinates and immutable, version-pinned search cursors.
Whisper millisecond starts round down; ends round up. Chunk offsets are added back
to source coordinates, and cue ends are bounded by the chunk/source duration. Empty
or whitespace-only whisper segments create no cue; silence is not fabricated as text.

At enqueue, the server records the current transcript version (including “none”).
Publication runs in one SQLite transaction and succeeds only if the job is still
`running`, the source is still present and `ready`, and that transcript version is
still current. A manual import or another publication during processing makes the
job `failed` and leaves the newer transcript untouched. Failure/cancellation never
publishes a partial transcript. Transcript publication does not change the project
edit revision or command history.

## Error envelope

Errors use the normal backend shape:

```json
{"error":{"code":"conflict","message":"..."}}
```

- `404 not_found`: project, source, or job does not match the route.
- `409 conflict`: runtime unavailable, source unsuitable, active-job collision,
  cancellation/publication race, or transcript/source changed before publication.
- `422 invalid_input`: malformed JSON, unsupported language, invalid persisted media
  bounds, malformed/oversized whisper output, a cue over 16 KiB, aggregate retained
  cue text over 32 MiB, or more than 20,000 cues.
- `500 internal` / `database`: local process, filesystem, or persistence failure.

## Worker command contract and limits

For each chunk the server executes FFmpeg with local file/format whitelists and
produces mono `pcm_s16le` at 16 kHz. It then executes the configured CLI as:

```text
<LONGCUT_WHISPER_CLI> -m <LONGCUT_WHISPER_MODEL> -f <chunk.wav> \
  -l <ko|en|auto> -t 4 -ng -oj -of <output-prefix> -np
```

Expected JSON is `transcription[]` with `offsets.from`, `offsets.to` in milliseconds
and `text`, plus `result.language`. JSON is limited to 16 MiB per chunk. Each trimmed
cue is limited to 16,384 UTF-8 bytes; the job may retain at most 33,554,432 UTF-8 cue
text bytes and 20,000 cues. Checked addition rejects overflow. The text and cue-count
budgets are checked before each cue is copied into the job accumulator, including
across chunk boundaries. Exceeding any limit fails the job before publication, so an
older transcript remains current and no partial ASR transcript exists.

Stderr is bounded. Chunks are processed sequentially inside each job; global active
jobs are bounded by configuration. Temporary WAV/JSON files are removed and no
whole-source WAV is retained or loaded into memory. Per-chunk cleanup failures are
propagated as job failures. Final work-directory cleanup failures are logged with the
job ID and path; they also fail an otherwise successful job before publication.

Alpha limitations: no model download or model management, no diarization/speaker
labels, no word timestamps, no retry/resume, no GPU/device controls, and no speech
accuracy claim. Automated backend tests use a fake CLI to verify command/persistence/
API behavior; they are explicitly not speech-accuracy tests. A real CLI/model speech
smoke test remains an installation validation step.
