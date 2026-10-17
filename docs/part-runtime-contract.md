# PART runtime migration contract

Integration target: exact `@slop-lab/part@0.1.0` and
`@slop-lab/part-plugins@0.1.0`, upstream commit
`c85038744bfef810551984e3e9da4d447839caab`. Runtime Node requirement: 24.22.0+.

PART owns each provider generation's agent run, model gateway invocation,
observation and cancellation. Longcut supplies a Codex CLI broker transport and a
vision-compatible Chat Completions encoder because the upstream bundled adapters
do not provide Codex account login or media input. Account management remains an
application-owned boundary, not a claimed native PART feature.

## Ownership

- Rust remains authoritative for consent, selected scope, image sampling, persisted
  AI run state, evidence validation, stale revisions and Ready ChangeSet publication.
- `services/agent-runtime` replaces `services/codex` as the private execution service.
  Both `codex` and `openai-compatible` generations go through PART; Rust no longer
  constructs external provider HTTP requests.
- No editing, filesystem, shell or network tool is advertised to the model. The
  existing explicit human ChangeSet application remains the only edit-application path.
- Codex retains the existing account volume, sandbox policy, event gate and isolated
  subprocess environment. API keys must never enter Codex child environments or
  model input. Provider credentials are supplied by explicit host callbacks.

## Private HTTP contract

Default port remains 4100. All routes except `/health` require the existing private
Bearer link token; unauthorized responses reveal no runtime/provider configuration.

- `GET /health`: `{status: "ok", engine: "@slop-lab/part", version: "0.1.0"}`.
- `GET /runtime`: `{engine: "@slop-lab/part", version: "0.1.0", node_version: string,
  protocol_version: 1}`.
- `GET /providers`: `{providers: ProviderStatus[]}`, preserving public fields
  `{id, name, configured, model, endpoint, detail}` and nullable values.
- `GET /status`, `POST /login`, `/login/cancel`, `/logout`: existing CodexStatus
  contract, restricted to the existing Codex account implementation.
- `POST /jobs`: accepts the existing `{id, prompt, schema, images}` plus
  `provider: "codex" | "openai-compatible"`. Omitted provider defaults to Codex for
  existing internal callers. Returns 202 with RuntimeJob after PART admission.
- `GET /jobs/{id}`, `POST /jobs/{id}/cancel`: return RuntimeJob.

```ts
type RuntimeJob = {
  id: string;
  status: "running" | "succeeded" | "failed" | "cancelled";
  result: Proposal | null; // existing closed proposal schema
  error: {code: string; message: string} | null; // sanitized application error
  runtime: {
    engine: "@slop-lab/part";
    engine_version: "0.1.0";
    execution_id: string; // actual PART handle.ref.id, never fabricated
    profile_revision: string;
  };
  progress: {
    sequence: number;
    phase: "admitted" | "provider" | "validating" | "cleaning_up" | "terminal";
  };
};
```

Single active generation/account operation is preserved. Terminal result retention
is bounded, and losing an observation is not cancellation. Cancellation explicitly
requests PART cancellation and waits for owned transport cleanup/reaping. A timeout
never permits a late result to become a proposal.

Runtime progress uses a fixed allowlist. Never expose raw provider events, prompts,
text deltas, credentials, subprocess output or provider error bodies. PART's default
response/cancellation limits do not replace Longcut's bounded transport output,
ten-minute generation deadline and explicit subprocess cleanup ownership.

## Rust/public compatibility

Existing `AiRequest`, provider IDs/defaults, persisted `AiRun`, Codex routes and
ChangeSet semantics remain compatible. Both providers use one AgentRuntimeClient.
Canonical configuration names are `LONGCUT_AGENT_RUNTIME_URL` and
`LONGCUT_AGENT_RUNTIME_TOKEN_FILE`; existing `LONGCUT_CODEX_URL` and
`LONGCUT_CODEX_TOKEN_FILE` are fallback aliases. Keep the `codex` service network
alias and account volume when renaming the Compose service.

Add separate optional observation resources rather than rewriting persisted runs:

- `GET /api/v1/ai/runtime` returns
  `{available: boolean, engine: "@slop-lab/part", version: string|null, error: string|null}`.
- `GET /api/v1/projects/{project}/ai/runs/{id}/runtime` first verifies the run belongs
  to the project, then returns
  `{run_id: string, available: boolean, runtime: RuntimeJob.runtime|null,
  progress: RuntimeJob.progress|null}`. Evicted/legacy runtime jobs return
  `available: false`; they do not invalidate the persisted proposal or ChangeSet.

Provider configuration names `OPENAI_API_KEY`, `OPENAI_BASE_URL`, `OPENAI_MODEL`,
`OPENAI_RESPONSE_FORMAT`, `OPENAI_MAX_OUTPUT_TOKENS`, and `OPENAI_TOKEN_PARAMETER`
are consumed by the runtime service, not by Rust or Codex. Configuration comes from
explicit process settings; implementation and QA do not read `*.env` files.

## Required execution evidence

Tests must execute real PART against loopback HTTP fixtures and a fake Codex binary
(not mock PART itself). Preserve request/image/schema compatibility, auth isolation,
bounded JSON/SSE decoding, redirect rejection, tool/truncation rejection,
process-group cancellation, cleanup, stale-result protection and human approval.
These fixtures prove integration behavior, not a paid-provider connection or
successful ChatGPT subscription inference. Production images install exact packages
and a supported Node version. No general PART browser or node host package is needed.
