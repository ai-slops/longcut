# Local ASR and timing validation

This records implementation evidence, not Stage 1 release acceptance.

## Real speech, not a fake CLI

The native server was exercised with the actual CPU whisper.cpp executable and
multilingual `tiny` model. Both English and Korean jobs published immutable
`local_asr` transcripts without advancing the project edit revision. Cancelling a
subsequent job retained the previous transcript.

- Runtime: whisper.cpp `1.9.4-dev`, extracted from official image
  `ghcr.io/ggml-org/whisper.cpp@sha256:0d569c8cfd664dd256df2f881b9ae7881326a1ab9fd3ba4ccb7b6f0198f46393`.
- Model SHA-256: `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`.
- English input: official whisper.cpp `samples/jfk.wav` at source revision
  `60c0be6ac8fa71b1a2ae2dd938a31a34a508e774`. The result included the expected
  “ask not what your country can do for you” passage.
- Korean input: [Google FLEURS](https://huggingface.co/datasets/google/fleurs),
  `ko_kr`, test row 0, ID 1883, dataset revision
  `70bb2e84b976b7e960aa89f1c648e09c59f894dd` (CC BY 4.0).
  Reference begins “다리 밑 수직 간격은 15미터이며”. Tiny produced
  “다름이 수직강격은 15mAh”. This visibly inaccurate result demonstrates why the
  smoke must not be presented as Korean production-quality acceptance.

The browser ran a real Korean job to `ready`, refreshed the transcript, displayed
engine/model/language and “Machine-generated · not human verified”, searched the
recognized text and selected its source range without moving Output. Captures:
`artifacts/asr-final-{1280,768,375}.png`.

## Three-hour synthetic input

A generated 16 kHz mono silent FLAC was imported through the running API:

- Duration: 10,800 seconds, 172,800,000 audio samples, 324,000 editor frames at 30 fps.
- Managed input: 2,040,576 bytes; ready proxy: 1,362,047 bytes; waveform: 330 points.
- Observed import-to-ready: approximately 7 seconds in this container.
- Proxy suffix request returned HTTP 206 and bytes `1361983-1362046/1362047`.
- ASR configured with 30-second chunks reported 360 chunks, then was cancelled
  before completing the first chunk. The transcript endpoint remained JSON `null`.
- Project: `5f0717f1-fa9c-4840-ae25-bcde88a95692` in isolated QA storage.

This proves synthetic long-duration ingest, bounded job planning and cancellation,
not three hours of transcription accuracy, video decode performance, AV alignment,
or real-broadcast acceptance. Full long-recording ASR was intentionally not run.

## Timing and regression gates

Real FFmpeg tests import an original with video start `60/30` and audio start
`96000/48000`, normalize it to zero, retain the original endpoints, and expose a
10-second/300-frame editable source. Unequal AV origins and mapped interior ranges
are rejected rather than silently approximated. Fractional frame durations use
exact ceiling; 1.05 seconds at 30 fps produces 32 frames. Failed adapters clean
their isolated intermediate files.

Workspace Rust tests, formatting, Clippy with warnings denied, and native build
passed. Browser tests passed 37 cases and TypeScript checking passed. Regression
coverage includes ASR restart failure, manual-transcript publication conflicts,
actual process cancellation/reaping, and pre-retention resource bounds: 16 KiB per
cue, 32 MiB total text, 20,000 cues and 16 MiB JSON per chunk.

Final independent UI review passed after correcting failed-cancellation polling,
Korean line wrapping, tablet pane flow and narrow-layout touch targets. The language
selector measures 44px at both 768px and 375px; document width exactly matches the
1280px, 768px and 375px viewports. No horizontal-overflow cropping was used.

## Configuration and remaining limits

See [ASR API](asr-api.md) and [UI workflow](asr-ui.md). Nonsecret runtime paths,
fixture metadata and QA evidence belong in JSON/TOML/Markdown, not `*.env` files.
Pass runtime settings directly in the process environment; no `.env` read is needed.
Model binaries and downloaded audio are not committed.

Remaining: representative Korean quality evaluation with larger models, word/token
alignment, chunk-overlap reconciliation, resumable jobs, arbitrary drift/gap
retiming, and the two licensed real 3+ hour broadcast/creator acceptance scenarios.
