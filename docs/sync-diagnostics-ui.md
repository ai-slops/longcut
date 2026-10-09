# Sync diagnostics UI

The Sources workspace exposes **Sync diagnostics** only on a committed
`SourceSessionSummary`. The disclosure is collapsed by default and lazily requests:

`GET /api/v1/projects/{project}/source-sessions/{session}/sync-diagnostics`

Prepared or validating imports do not expose the control because the read endpoint
uses active-project source-session membership. Undo may therefore make a previously
committed session and its diagnostics unavailable; redo may make the preserved record
visible again.

## Browser contract

The browser parses the endpoint independently from the session-import summary. Its
Zod schema mirrors `docs/sync-diagnostics-api.md`: tagged exact values, coverage
variants, source/binding/segment diagnostics, overlaps, limits, capabilities, and
health notes. A response outside that contract is an ordinary panel error and is not
rendered as partial success.

The panel always says **Declared metadata diagnostics** and states that TimeMap
relations are not measured or verified alignment. It must not use “synced”, “verified
alignment”, or “media repaired”. `confidence` is displayed only as declared
confidence.

## Exact values

All exact quantities remain decimal numerator and denominator strings from the API.
Rendering uses the literal fraction form `numerator/denominator`; it never parses a
rational through `Number`, converts it to a decimal, or rounds it. The compact segment
row shows:

- declared session range;
- offset at start;
- duration delta (drift over that segment);
- relative rate ratio and relative-rate ppm;
- declared confidence.

Unknown and unverifiable variants retain their status and reason. In particular,
missing TimeMaps display `Unknown — time maps missing`; this is ordinary absent
metadata, not evidence that streams are aligned.

## Coverage and mappings

Exact coverage is labelled **Coverage union**. Its range count refers to the merged
union of all declared TimeMap session ranges, clamped to the declared session domain.
It is not per-stream coverage and must not be described as every stream covering the
session. Gaps are metadata observations, not proof of damaged media.

Unsupported editor mappings reuse the committed session summary's producer-supplied
reason. A missing reason falls back to “Editor mapping unavailable” rather than
inventing a timing diagnosis. Unsupported mappings own the same independent 40-row
incremental paging contract as diagnostic ranges and segments; none are silently
discarded.

## Bounded rendering

The response may describe up to 20,000 diagnostic items. The UI may flatten the
already parsed segment metadata in memory, but each segment, coverage-gap, and
same-stream-overlap section initially mounts at most 40 rows. Unsupported source
mappings also mount at most 40 rows initially. Each section owns its
own **Load 40 more** control, so expanding gaps does not expand overlaps or segments.
Long gap and overlap sections start collapsed, but no valid declared item is silently
discarded. This bounds DOM work while preserving an explicit path through the full
declared dataset. Time ranges, offsets, and duration drift are labelled in seconds;
relative-rate ppm is labelled `ppm`, while the relative-rate ratio remains unitless.

## ASR word timing alongside diagnostics

Local-ASR cues may include `words` with stable IDs and source-frame strings. A cue
with words gets a compact sibling disclosure. The cue selection button and disclosure
button are siblings, so the UI never nests interactive controls.

Expanded words are explicitly labelled **Model-estimated word timing · not verified
alignment**. Selecting a positive interval updates only the Source seek position and
the shared Source In/Out selection. It does not move the independent Output/Program
playhead. Equal start/end punctuation remains visible with its declared frame but is
not a button and cannot create a positive cut. Manual subtitle imports reject ASR-only
word arrays; the browser never fabricates words from cue bounds.

The sync section is labelled by its persistent disclosure button, so the accessible
name remains valid while the body is collapsed. Sync and word disclosures also use
stable explicit accessible names; `aria-expanded` communicates state. Visible trust
copy and diagnostic values use at least the 12px small-text token. Range summaries
and word controls provide 44px targets on narrow or coarse-pointer layouts. Expanding
word timing raises the transcript scroll region to a readable bounded height instead
of constraining the details to the former compact cue-list height.

## Runtime QA evidence

The native gateway at `http://127.0.0.1:3005` was exercised against project
`b45fe57e-5fb4-4797-9cd6-54a809fa5736`. Fresh viewport captures retain the application
chrome and use distinct expanded-state context/bottom files:

- `artifacts/sync-diagnostics-expanded-{1280,768,375}-{context,bottom}.png`
- `artifacts/word-timing-expanded-{1280,768,375}-{context,bottom}.png`

At 1280/768/375px, measured horizontal document overflow was zero. Diagnostic values
and the word-timing trust label measured 12px. Narrow range summaries and word controls
measured 44px; desktop/tablet word controls remain the design-system 32px pointer
target. The expanded transcript list measured 269/288/288px, and the second ASR cue
visibly preserved two zero-duration units as non-buttons. The sync section's
`aria-labelledby` target resolved at every viewport.
