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

## Poll, cancel, and explicitly resume

- `GET /api/v1/projects/{project_id}/sources/{source_id}/asr/{job_id}`
- `POST /api/v1/projects/{project_id}/sources/{source_id}/asr/{job_id}/cancel`
- `POST /api/v1/projects/{project_id}/sources/{source_id}/asr/{job_id}/resume`

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
  resume_eligible: boolean;
};
```

Cancellation is idempotent for terminal jobs. For an active job the server first
persists `cancelled`, then signals the worker. An active FFmpeg or whisper.cpp child
is killed and reaped. No transcript is published. `resume_eligible` is true only for
a `failed` or `cancelled` schema-v5 job with a persisted resume snapshot. Legacy jobs
and `queued`, `running`, or `ready` jobs report false.

Resume has no request body and returns `202 Accepted` with the same job ID reset to
`queued`. It never starts automatically: a server restart still converts persisted
`queued` or `running` jobs to `failed`, preserves validated checkpoints, and waits
for this explicit endpoint. A resume request returns `409 conflict` while the old
in-process worker is still stopping. Worker registration never replaces an occupied
entry, so old and resumed workers cannot overlap. Concurrent requests permit exactly
one transition; another active job on the source also returns `409`.

Every new job privately snapshots streamed SHA-256 digests for the managed source,
configured model, and configured CLI executable, plus the source media URL, duration
frames, project frame rate, requested language, chunk/overlap settings, and conversion
and CLI-argument policy versions. These digests are not returned by the API. Before
resume, the server re-hashes all three files with a bounded buffer outside the SQLite
transaction. Inside the resume transaction it requires an exact snapshot match,
unchanged ready source metadata, and the original base transcript version. Any source,
model, CLI, policy, or transcript change returns `409` without changing job state.
The source, model, and CLI executable therefore must remain immutable while a job is
active. The CLI digest covers the configured wrapper/executable file only; it does not
guarantee that dynamically linked libraries or other runtime dependencies are unchanged.

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
    result_language: string | null,
    chunk_seconds: number,
    overlap_seconds: 2,
    ownership_policy: "half_open_midpoint",
    timing_quality: "model_estimated"
  }
}
```

Manual imports remain `origin: "subtitle_import"` and omit `asr`. Cues use the same
half-open source-frame coordinates and immutable, version-pinned search cursors.
ASR cues additionally contain optional `words`; the exact shape is documented in
[transcript-api.md](transcript-api.md). Word IDs are stable for the immutable
transcript version. Their source-frame times are whisper.cpp model estimates, not
verified alignment or forced alignment.

Whisper millisecond starts round down and ends round up. Each canonical chunk owns
the half-open range `[chunk_seconds * i, chunk_seconds * (i + 1))`, clipped to the
source. Inference extends two seconds before and after that range, also clipped to
the source. Local offsets are translated from the inference-window start. A word is
owned where twice its midpoint is within the canonical half-open owner range; an
exact midpoint tie therefore belongs to the chunk on the right. Reconciliation
compares candidates only with retained words from the immediately preceding
inference chunk. Its positive-duration ranges are indexed once by normalized text,
sorted, and strictly overlapping ranges are merged. Positive candidates then use a
logarithmic interval lookup. A candidate is suppressed only when normalized text
matches and positive estimated source ranges strictly overlap; touching ranges and
zero-duration point units are never deduplicated. Repetitions emitted by the same
inference remain distinct even when their model ranges overlap. This bounded policy
cannot guarantee semantic deduplication when neighboring inferences disagree about
text or timing.

Empty or whitespace-only results create no word. A model-emitted zero-duration
lexical word or punctuation unit keeps equal start/end frames when it can join a cue
containing positive-duration words; duration is never invented and consumers must
not treat that unit as a selectable interval. A group containing only zero-duration
results fails the job explicitly because it cannot form a positive-duration cue;
such words are never silently discarded. Reversed ranges always fail. Coordinates
and ownership are computed with checked arithmetic before strict inference bounds:
an unowned context candidate is discarded even if the model extends it beyond the
extracted window, while an owned candidate outside that window still fails the job.
Owned timing is never clipped or fabricated.

At enqueue, the server records the current transcript version (including “none”).
Publication runs in one SQLite transaction and succeeds only if the job is still
`running`, the source is still present and `ready`, and that transcript version is
still current. A manual import or another publication during processing makes the
job `failed` and leaves the newer transcript untouched. Failure/cancellation never
publishes a partial transcript. Transcript publication does not change the project
edit revision or command history.
The same publication transaction writes both compatibility `transcript_cues.words`
JSON and normalized `transcript_words` index rows, so readers never observe a cue
without its indexed ASR words.

After whisper JSON parses and the overlap accumulator accepts a chunk, the server
stores that validated JSON, its canonical inference/owner bounds, and progress in one
SQLite transaction. A checkpoint is limited to 16 MiB and all checkpoints for one job
to 128 MiB; both limits are enforced inside the transaction. Resume requires indexes
starting at zero without gaps, recomputes every canonical window, parses every payload,
and sequentially replays it through the same accumulator limits. This reconstructs the
immediately previous chunk state before invoking whisper only for remaining chunks.
Consequently resumed output has the same cue/word text and timing as uninterrupted
execution; newly generated immutable UUIDs may differ. Corrupt, gapped, oversized, or
policy-incompatible checkpoints return `409` without queueing the job. Successful
transcript publication deletes all job checkpoints in the same transaction. Failed,
cancelled, and restart-interrupted jobs retain them.

The current project database schema is v8. The v6 upgrade from v5 backfills missing
checkpoint fingerprints from bounded, ordered checkpoint reads; v7 adds immutable
Moment versions; v8 transactionally backfills indexed transcript words while retaining
the compatibility JSON. Resume snapshots remain a v5-origin capability, so pre-v5
jobs without one stay ineligible.

## Error envelope

Errors use the normal backend shape:

```json
{"error":{"code":"conflict","message":"..."}}
```

- `404 not_found`: project, source, or job does not match the route.
- `409 conflict`: runtime unavailable, source unsuitable, active-job collision,
  cancellation/publication race, in-process worker still stopping, ineligible resume,
  changed resume inputs/runtime/policy/transcript, invalid checkpoint replay, or
  transcript/source changed before publication.
- `422 invalid_input`: malformed JSON, unsupported language, invalid persisted media
  bounds, malformed/oversized whisper output, a zero-duration-only cue group, a cue
  over 16 KiB, aggregate retained cue text over 32 MiB, or a word/candidate/count
  limit.
- `500 internal` / `database`: local process, filesystem, or persistence failure.

## Worker command contract and limits

For each chunk the server executes FFmpeg with local file/format whitelists and
produces mono `pcm_s16le` at 16 kHz. It then executes the configured CLI as:

```text
<LONGCUT_WHISPER_CLI> -m <LONGCUT_WHISPER_MODEL> -f <chunk.wav> \
  -l <ko|en|auto> -t 4 -ng -ml 1 -sow -oj -of <output-prefix> -np
```

Expected JSON is `transcription[]` with `offsets.from`, `offsets.to` in milliseconds
and `text`, plus `result.language`. `-ml 1 -sow` makes regular `-oj` segments the
whole-word units; full token JSON is not consumed, avoiding byte/token fragments for
Korean. JSON is limited to 16 MiB per inference window. The job processes at most
300,000 nonempty word candidates, including unowned overlap context and duplicates;
this cap applies before ownership or reconciliation, so discarded candidates cannot
bypass the job bound. It may retain at most 100,000 words, 33,554,432 UTF-8 word-text
bytes, 20,000 readable cues, and 33,554,432 UTF-8 cue-text bytes. A word and a cue are
each limited to 16,384 UTF-8 bytes. Cues contain at most 12 words and split earlier
to honor the byte limit, so 100,000 short words require at most 8,334 cues. Checked
arithmetic rejects overflow. Exceeding any limit fails the job before publication,
so an older transcript remains current and no partial ASR transcript exists.

Stderr is bounded. Chunks are processed sequentially inside each job; global active
jobs are bounded by configuration. Temporary WAV/JSON files are removed and no
whole-source WAV is retained or loaded into memory. Per-chunk cleanup failures are
propagated as job failures. Final work-directory cleanup failures are logged with the
job ID and path; they also fail an otherwise successful job before publication.

Alpha limitations: no model download or model management, no diarization/speaker
labels, no forced alignment, no automatic retry/resume, no GPU/device controls, and no speech
accuracy or guaranteed overlap-deduplication claim. Automated backend tests use a
fake CLI to verify command/persistence/API behavior; they are explicitly not
speech-accuracy tests. A real CLI/model speech smoke test remains an installation
validation step.
