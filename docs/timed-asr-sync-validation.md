# Timed ASR and declared synchronization diagnostics

## Implemented behavior

Local ASR now expands each canonical chunk by two seconds on either side, clipped
to the source. Whisper's word-segment mode supplies model-estimated time ranges.
Half-open midpoint ownership assigns candidates to chunks; a normalized-text,
sorted-interval index reconciles positive-duration overlaps against the preceding
chunk. Repeated words within the same inference remain distinct. This is a bounded
heuristic, not proof of recognition accuracy or independently verified alignment.

Words have stable IDs and source-frame ranges. Up to twelve units form a readable
cue; word timing survives SQLite reopening and paginated transcript search. Existing
subtitle imports and populated version-3 databases retain their cue text and have
no invented word timing. The schema migration adds nullable word storage.

Zero-duration model units remain non-selectable in a positive-duration cue. If a
nonempty zero-only group cannot form a valid cue, the job fails explicitly rather
than dropping text or inventing a duration. Retention and processing are bounded:
100,000 retained words, 300,000 nonempty candidates, 20,000 cues, 16 KiB per unit/cue,
and separate 32 MiB word/cue text budgets. Prior-chunk lookup is indexed rather than
quadratic in previous and current candidate counts.

The new source-session endpoint and panel expose declared offsets, duration drift,
rate ratios, ppm, coverage-union gaps and same-stream overlaps. Values remain
reduced integer-string fractions. Unknown metadata and calculation failures are
explicit, and projected response expansion is capped before materialization.
An editable ordinary source may still have unknown exact TimeMaps. Diagnostics do
not alter media, retime playback or assert measured audio/video alignment.

## Real execution evidence

A public Korean FLEURS sample repeated four times yielded approximately 50 seconds
of audio. Through the real native server and whisper.cpp tiny model:

- 30-second owner chunks with two-second context completed **2/2 chunks**.
- Result: **5 cues, 52 word units**, including two zero-duration lexical units.
- All retained word ranges stayed within the source and were non-reversed.
- Browser word selection set Source/In to **910**, Out to **962**, while Output
  remained **0**.
- Project: `b45fe57e-5fb4-4797-9cd6-54a809fa5736` in isolated QA storage.
- Final checked job: `d66123d2-9965-4426-b147-458549467d23`.

Real execution caught a Whisper estimate `[31.480, 41.420]` in a 32-second inference
window. Its midpoint belonged outside the current owner chunk. The worker now
discards that unowned context candidate before validating retained-word bounds;
owned out-of-window results still fail explicitly. This is not silent timestamp
clipping.

The same browser imported the synthetic complete package and fetched real sync
diagnostics: exact domain `0/1–10/1` seconds, four mapped-stream rows, zero drift,
unit rate, and an explicit unsupported microphone adapter. Ordinary-source
diagnostics returned unknown TimeMaps instead of claiming synchronization.
Controlled browser-response fixtures separately verified segment expansion from
40 to 54 rows and gap expansion from 40 to 51; those fixtures are not media evidence.

## Directional Korean model comparison

The first four public `google/fleurs`, `ko_kr/test` rows (0–3) were decoded with the
same whisper.cpp `1.9.4-dev` CLI, CPU four-thread settings and `-l ko -ng -oj -np`.
Dataset asset revision: `70bb2e84b976b7e960aa89f1c648e09c59f894dd`.

| Model | Total wall time for 44.76 s audio | Observation |
| --- | --- | --- |
| multilingual tiny | 2.399 s | Wiring/smoke model; clear Korean errors remain |
| multilingual base | 4.608 s | Better on several phrases, not every phrase; about 1.92× latency |

Base SHA-256:
`60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe`.
Tiny hash and runtime provenance are recorded in [ASR validation](asr-validation.md).
Times include model loading and process startup. Four read-speech samples do not
establish broadcast accuracy, and no default model was automatically promoted.
The quantized base-q5_1 attempt failed with SIGILL on this CPU/build combination;
full-precision base succeeded. Model compatibility must be tested on target hardware.

## Validation and remaining scope

The final workspace tests, format check, all-target Clippy with warnings denied,
native build, browser tests and TypeScript/Vite build pass. Tests cover overlap
ownership, same-inference repetitions, context overruns, zero-only failure, indexed
reconciliation, word migration/persistence, exact arithmetic and projected diagnostic
output limits. Current API details: [ASR](asr-api.md),
[transcripts](transcript-api.md), [sync diagnostics](sync-diagnostics-api.md).

The final browser suite passed 53 tests. Independent reviews approved the indexed
ASR conversion and the final UI. Final UI evidence comprises context and lower-detail
captures for each 1280/768/375px viewport:
`artifacts/word-timing-expanded-{width}-{context,bottom}.png` and
`artifacts/sync-diagnostics-expanded-{width}-{context,bottom}.png`.
These preserve app chrome and show the expanded content rather than hiding clipped
scroll regions. The UI keeps all gap, overlap and unsupported-source rows reachable
through bounded pagination; same-stream channel bindings have distinct rendering IDs.
Rust diagnostics were clean on the changed modules. TypeScript LSP remains unavailable;
the compiler and production build are the TypeScript validation evidence.

Still outstanding: resumable analysis jobs, independently verified/forced alignment,
general drift and discontinuity retiming, semantic story analysis, and representative
licensed multi-hour broadcast/creator acceptance. The existing synthetic long-audio
test and this short repeated-speech test do not discharge those release gates.
Nonsecret evidence uses JSON and Markdown; no `*.env` access is needed.
