# PART runtime UI

The Agent workspace reports PART as execution infrastructure. It does not use runtime
availability as a proxy for provider configuration, ChatGPT account state, billing,
or consent.

## Public resources

- `GET /api/v1/ai/runtime` supplies the nonblocking runtime availability panel. The
  browser accepts only `available`, the fixed `@slop-lab/part` engine name, `version`,
  and a sanitized `error`.
- `GET /api/v1/projects/{project}/ai/runs/{id}/runtime` is requested only after a
  user expands **Runtime details** for a saved run. The browser accepts only the run
  ID, availability, PART engine/version, execution ID, profile revision, and the
  fixed progress sequence and phase.
- `PublicAiRun` remains unchanged. Runtime observation is optional and does not alter
  a saved proposal or its ChangeSet.

Zod strips undeclared response fields. Unknown progress phases are rejected rather
than displayed. The UI has no schema or rendering path for prompts, text deltas,
provider payloads, subprocess output, credentials, API keys, or tokens.

## Visible states

The runtime panel says **available** only when the public availability resource says
so and shows the reported engine version. An unavailable or failed check remains an
honest warning while the manual editor, provider selector, Codex ChatGPT login, and
existing ChangeSet review remain usable.

Per-run details are a lazy disclosure. Active `queued`, `sampling`, or `generating`
runs poll every two seconds while expanded. Terminal runs load once. Closing the
disclosure, changing project/run identity, or unmounting the Agent workspace aborts
the in-flight request and clears its timer. A response with the wrong run ID is
rejected. Legacy, restarted, or evicted observations render a nonfatal explanation;
they never remove or invalidate a proposal.

Progress is deliberately a phase plus monotonic sequence counter, not a percentage:
`admitted`, `provider`, `validating`, `cleaning_up`, or `terminal`.

## Provider and approval boundaries

The provider selector remains explicit and never falls back silently. OpenAI-compatible
configuration exposes only existing public status, model/endpoint metadata, and the
names `OPENAI_API_KEY`, `OPENAI_MODEL`, and `OPENAI_BASE_URL`; it never accepts or
persists values in the browser. API billing disclosure remains separate from ChatGPT
subscription use. Codex device login remains its own account panel.

Runtime availability does not bypass the context disclosure or consent checkbox.
Generated edits still become a reviewable ChangeSet and are never auto-generated on
page load or auto-applied.

## Integration and QA

The browser requires the public resources defined in
[`part-runtime-contract.md`](part-runtime-contract.md). A backend that predates those
routes will correctly display the runtime as unavailable; it cannot provide live PART
proof until the backend/service stack is refreshed. Controlled wire fixtures may be
used to prove the available and per-run rendering states, but must not be reported as
a paid-provider connection or live PART execution.

The supported runtime baseline is Node 24.22.0+ with PART 0.1.0. No PART browser SDK
is installed or required.
