# Historical AI integration validation

Date: 2026-09-20. This report preserves evidence from the pre-PART provider
integration checkpoint. For the current shared runtime architecture and fixture
acceptance, see [the runtime guide](agent-runtime.md) and
[PART runtime acceptance](part-runtime-acceptance.md).

## Implemented paths

- Official Codex CLI 0.155.1 with ChatGPT device login and isolated credential storage.
- Explicit OpenAI-compatible API selection with server-only `.env` configuration.
- Source-interval and existing-scene scopes, opt-in context sharing, twelve-frame
  maximum sampling, durable jobs, cancellation, revision checks and partial approval.
- Insert, trim, camera, metadata/caption and delete proposals through the existing
  command engine. Neither provider applies its own proposal.
- Streaming Chat Completions decoding, strict proposal parsing, redirect rejection,
  output limits and rejection of tool calls, truncation and provider error envelopes.

## Automated evidence

- Rust: 55 passing tests, formatting check, Clippy with warnings denied, release build.
- Then-current Codex service: 30 passing tests and strict TypeScript checking.
- Web: 15 passing tests, strict TypeScript, Biome and production Vite build.
- Language-server diagnostics are unavailable; compiler/typechecker checks are used instead.

Provider tests used explicit local HTTP/CLI fixtures, never a production fallback.
They established wire format, parsing, scope checks, preconditions, cancellation
races, credential redaction and atomic publication. They did not involve a live
model or prove that a real vendor accepted a generation request.

## Real Codex checks

The actual pinned binary and SHA-256 were checked. Startup verified effective shell
feature disables, credential-read denial and write denial. This Docker runtime
rejects bubblewrap namespace creation; the adapter retains its restrictive policy
and accepts only tool-free inference, never a sandbox-bypassed fallback.

The real browser initiated the official device-login flow, displayed the OpenAI
authorization link and user code, and cancelled that flow successfully. Screenshots
redact the code. The user chose not to authorize ChatGPT at this time, so authenticated
Codex generation has not been demonstrated.

## Real configured API checks

The user supplied server-side configuration. Its values were not printed, copied into
prompts, placed in browser inputs or committed. The authenticated live-provider check
was limited to a models-listing request: it returned HTTP 200 and included the
configured model. No live inference request or real-model editing result was used as
validation evidence.

The model-catalog result proves neither generation access nor image/schema
compatibility. Current proposal-path evidence remains fixture-based unless a separate
report explicitly records authenticated inference.

## Operations

Only application services were rebuilt/recreated. No DIM workspace restart was used.
Project, backup and Codex-auth volumes were preserved. Completed temporary build/test
containers and explicitly identified stale QA images were removed to free Docker
storage; no volume or general Docker-system prune was performed.
