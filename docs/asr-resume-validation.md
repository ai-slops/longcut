# ASR checkpoint and resume validation

## Behavior

ASR jobs now persist validated inference results and progress together in SQLite.
After failure, cancellation or process restart, an eligible job can explicitly
resume from retained chunks. Resume revalidates source/model/configured-CLI hashes,
source timing, chunk/conversion policy and the original transcript version. Changed
inputs or a newer transcript reject continuation without replacing project data.
Legacy jobs without a resume snapshot cannot reuse work and must start a new job.

Resume reconstructs overlap-reconciliation state by replaying retained results;
it does not rerun inference for committed chunks. Publication and checkpoint deletion
are atomic. Raw checkpoint storage is capped at 16 MiB per chunk and 128 MiB per job,
with size preflight before payload allocation and sequential replay outside the
short activation transaction. Progress, count, windows and chained fingerprints
are rechecked before activation.

Database schema v6 handles populated v5 jobs both with and without the fingerprint
column, backfilling missing chains transactionally while preserving nonempty chains.
Older manual transcripts and non-resumable jobs remain readable.

## Actual process crash

The test used the real native server, real whisper.cpp tiny model and approximately
50 seconds of repeated public Korean FLEURS speech. A QA-only wrapper recorded CLI
invocations and delayed launch by two seconds to make interruption reproducible;
it executed the actual recognizer and never supplied fake transcription output.

1. Job `8c411f62-7df0-4fdc-9b54-f2cd15a91151` reached checkpoint **1/2**.
2. The isolated server process was terminated with **SIGKILL**.
3. The same database was reopened with the same source, model and CLI configuration.
4. GET reported `failed`, progress **1/2**, and `resume_eligible: true`.
5. Explicit POST resume returned **202**, then completed **2/2**.
6. The result contained **5 cues and 52 words** within the source bounds.
7. Invocation records showed chunk 0 ran **once**; interrupted chunk 1 ran again.
8. Successful publication left **zero** checkpoints for the job.

Attempt scratch paths are `asr-work/{job_id}/{attempt_uuid}`. A surviving subprocess
from a terminated attempt cannot write into a later attempt's output path.

## Real browser continuation

The lead reopened the QA database using the final schema-v6 server and continued
cancelled job `053c1b0e-c28e-4c92-beaa-067205a65403` using **Resume transcription**.
The UI changed from **cancelled 1/2** to **ready 2/2** and refreshed the real transcript.
The job ID was restored into browser storage to reproduce a returning browser session;
job status and inference results came from the live API, not response fixtures.

On the final UI build, the lead also started a new job from the browser, cancelled
after the first checkpoint, and resumed it. An immediate attempt returned the
expected **409 previous worker is still stopping**; retry after cleanup succeeded.
This proves cancellation does not permit simultaneous old/new workers.

Captures of actual eligible/ready states at 1280, 768 and 375px:
`artifacts/asr-resume-{eligible,ready}-{width}.png`.
Document width matched viewport width at each size. Independent final UI review
passed. The controls explain that inputs are revalidated before continuation, never
resume automatically, and announce status changes through polite live regions.

## Regression checks

- Full Rust workspace tests, formatting, all-target Clippy with warnings denied,
  and native build passed.
- Web tests: **61 passed**; TypeScript and production build passed.
- Resume-versus-cancel, concurrent resume, request-future cancellation, invalid
  source URL cleanup and per-attempt scratch isolation are covered.
- Corrupt/gapped/oversized checkpoints and mismatched progress/total counts reject
  activation without silently repairing or discarding saved work.
- Source/model/CLI/policy changes and a newer manual transcript reject resume.
- UI delayed-action tests cover mutual exclusion, response identity, stale source
  completion, 409 recovery and unavailable runtime. Controlled UI fixtures used
  for these races are separate from the real inference evidence above.

## Limits

Exactly one server process must own a data directory. Configured runtime files must
not change while a job runs. CLI-file hashing does not fingerprint every dynamically
loaded library. Fingerprinting large media/models adds startup/resume latency.
Checkpoints for failed/cancelled jobs are retained until successful publication;
quota management and a user-facing discard action remain follow-up work.
This feature resumes ASR, not every media/render/analysis job in the roadmap.
It does not establish speech accuracy, forced alignment or real-broadcast acceptance.
All nonsecret QA configuration and evidence used JSON/Markdown, not `*.env`.
