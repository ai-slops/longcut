# Codex subscription editing contract

## Provider and trust boundary

Use the official OpenAI Codex CLI, pinned to `@openai/codex` 0.155.1, with
ChatGPT account authentication. The Codex path does not accept API keys, custom OAuth clients,
provider endpoints, or copied tokens through the editor. A separate explicitly selected
OpenAI-compatible API provider is defined below. Subscription availability
and usage limits are controlled by OpenAI. Account authorization is user-driven.

Codex is a proposal generator, never a direct project writer. A private sidecar
receives only the explicitly selected scope, source/scene metadata, and at most
12 timestamped JPEG samples. It has no project database or original-media mount.
The user must consent to sending this context to OpenAI before starting a run.
No ASR is implied: the initial feature understands supplied instructions and sampled
visuals, not unseen action or speech. Display this limitation before submission.

Rust owns source sampling, scope validation, durable run status, revisions, locks,
and proposal publication through the existing command engine. A generated proposal
never applies itself. The existing partial-approval workflow applies selected operations.

## Core HTTP API

Prefix: `/api/v1`.

### Account connection

- `GET /codex/status`
- `POST /codex/login` (starts official device login)
- `POST /codex/login/cancel`
- `POST /codex/logout`

All return `CodexStatus`:

```text
{
  installed: boolean,
  version: string|null,
  state: "logged_out"|"login_pending"|"ready"|"busy"|"error",
  auth_mode: "none"|"chatgpt"|"unsupported",
  login: {url: string, code: string, expires_at: string}|null,
  error: string|null
}
```

`expires_at` is the adapter's login-session deadline, not a claim about provider
token expiry. Never include auth-file contents, refresh tokens or raw process logs.
Unavailable sidecar errors use the existing `{error:{code,message}}` envelope.

### AI editing runs

- `GET /projects/{id}/ai/runs` returns `AiRun[]`, newest first, bounded to 50.
- `POST /projects/{id}/ai/runs` accepts the request below and returns `AiRun` with 202.
- `POST /projects/{id}/ai/runs/{run_id}/cancel` returns `AiRun`.

```text
AiRequest {
  base_revision: string,
  instruction: string,                    // 1..8000 characters
  consent: true,
  scope:
    {type: "source", source_id: string, start_frame: string, end_frame: string}
    | {type: "scenes", scene_ids: string[]} // 1..12 unique existing scenes
}
AiRun {
  id: string, project_id: string, base_revision: string,
  instruction: string, scope: AiRequest.scope,
  status: "queued"|"sampling"|"generating"|"ready"|"error"|"cancelled"|"stale",
  created_at: string, updated_at: string,
  change_set_id: string|null,
  summary: string|null,
  warnings: string[],
  sample_count: number,
  error: string|null
}
```

Source mode can propose appended scenes from the selected source interval. Existing
edits remain untouched. Scene mode can propose trim, camera, title/purpose/caption,
or deletion of explicitly selected scenes. It cannot insert other sources, change
locks, alter project settings, or modify objects outside the selected scope.

The source interval uses the existing normalized 30/60-fps frame contract, is
nonempty, bounded by actual source duration, and is at most six hours. Sparse
sampling is disclosed for long ranges. Scene metadata may include available camera
IDs and durations, but original audio/video is never sent to the sidecar.

An AI run is operational state outside project undo/revision history. On restart,
unfinished runs become errors with an actionable retry message. Publication must
atomically check active run state and its frozen project revision, execute the
normal CreateChangeSet command, and mark the run ready. If cancelled or stale,
publish no ChangeSet. A ready result with zero operations may retain a summary and
warnings without a ChangeSet. Never silently regenerate against a newer revision.

## Model response

Rust supplies a closed JSON Schema to the sidecar for this response:

```text
AiProposal {
  title: string, summary: string, warnings: string[],
  operations: AiOperation[] // maximum 24
}
```

Every operation includes `rationale: string` and `evidence_ids: string[]` referencing
only evidence actually supplied in the frozen context. Sample evidence IDs are
`frame_0001` etc.; metadata evidence uses `source_<id>` and `scene_<id>`.

| type | fields in addition to rationale/evidence_ids |
| --- | --- |
| insert_scene | source_id, in_frame, out_frame, title |
| trim_scene | scene_id, in_frame, out_frame |
| set_camera | scene_id, camera (existing Camera shape or null) |
| update_scene | scene_id, title, purpose, caption |
| delete_scene | scene_id |

Rust rejects unknown fields/variants, invented IDs/evidence, invalid ranges,
out-of-scope operations, locked objects, excessive output, and more than one
operation for the same existing scene/property. All new IDs are server-generated.
The source-mode insert order is the proposed output order.

## Existing ChangeSet extension

Keep shipped trim/camera operation payloads readable. Extend `OperationType` with
`insert_scene`, `update_scene`, and `delete_scene`; add optional source_id, title,
purpose, caption fields to the existing serialized operation struct. For insert,
`scene_id` is the server-generated ID assigned before proposal creation. Require
its absence from the current project when applied, and retain that ID on insertion.

Capture property preconditions for updates, full-scene preconditions for deletion,
and source/range plus new-ID preconditions for inserts. Validate every selected
operation against a candidate project before committing. Direct edits and locks
remain authoritative. Extend frontend Zod variants for null Option serialization.

## Private sidecar API

The sidecar listens on port 4100 inside the Compose network; no published port.
Rust uses `LONGCUT_CODEX_URL=http://codex:4100`. Authenticated endpoints require a
random bearer token stored at `/run/codex-link/token` in a dedicated shared volume.
The sidecar creates the token if absent with mode 0600. Rust mounts this volume
read-only. Codex tools must be denied access to this directory.

- `GET /health`: tool/version readiness only; no auth metadata.
- `GET /status`, `POST /login`, `POST /login/cancel`, `POST /logout`: account API above.
- `POST /jobs`: `{id: UUID, prompt: string, schema: JsonObject, images: string[]}`.
  Images are JPEG base64, at most 12; maximum request body 16 MiB. Returns `Job`.
- `GET /jobs/{id}`, `POST /jobs/{id}/cancel`: return `Job`.

```text
Job {
  id: string,
  status: "running"|"succeeded"|"failed"|"cancelled",
  result: AiProposal|null,
  error: string|null
}
```

Serialize login/logout/generation: one Codex operation at a time per account.
Allow lightweight job polling while generation runs. Do not inspect auth files in
Longcut; determine account mode from the official CLI's login-status command.
API-authenticated sessions are unsupported and cannot start editing jobs.

## Execution policy

Use fixed executable/argv, `shell: false`, a generated per-run temporary directory,
an allowlisted environment and bounded stdout/stderr. Pin and check the CLI version.
Use `exec --ephemeral --json --output-schema ... --output-last-message ...`,
`--ignore-user-config`, `--ignore-rules`, `--strict-config`, forced ChatGPT login,
and approval policy `never`. No inherited API keys or model-provider overrides.

Use a named restricted permissions profile, not a conflicting legacy sandbox flag:
minimal platform files and the curated run directory are readable; `/auth`,
`/run/codex-link`, and `/proc` are explicitly denied; no model-tool paths are writable
and model-tool networking is disabled. Disable shell capability, code mode,
image-file tools, web search, hooks, MCP and skills using verified versioned config.
The built-in patch tool remains in this Codex version but must have neither read
access to credentials nor write access anywhere. Never bypass sandboxing, add host
sockets, grant privileged mode, or broaden permissions as a fallback.

Run timeout is ten minutes. Cancel first marks the job cancelled, then kills its
process group; late output cannot change that terminal state. Remove per-run files
after completion. Retain only a bounded number of sanitized terminal job records.
Tokens remain only in the separate auth volume owned by the Codex service account.

In 0.155.1, `unified_exec` is an implementation selector that is normalized to true,
not a shell capability switch. The pinned tool planner's `shell_tool=false` gate
removes both `exec_command` and `write_stdin`; startup checks that capability flag.
The adapter additionally rejects every tool-bearing JSONL event, including patch,
plan, execution, web and MCP items. The entire run is discarded after any tool attempt.

If the container denies bubblewrap namespace creation, the named deny policy is
retained and all tool execution stays unavailable. This is a stricter no-tool mode,
not a sandbox bypass. Tool-free authenticated inference must still be verified
after the user's account authorization. Do not enable privileged mode, add kernel
capabilities or disable seccomp to make the feature run.

## Validation gates

Use an explicit fake CLI only in tests for argv/env, invalid output, size limits,
timeouts and cancellation races. Run the actual pinned CLI to check version,
configuration and sandbox canaries without credentials. Never call fixture output
an authenticated AI result. After the user signs in, require a user-initiated real
proposal and approval to establish the authenticated end-to-end claim.

No workspace restart is required. Rebuild/recreate only application services and
preserve all existing project and auth volumes.

## Additional provider: OpenAI-compatible API

The user explicitly requested this additional path after choosing not to authorize
ChatGPT yet. Preserve Codex subscription authentication unchanged. Never automatically
fall back from Codex to billable API calls.

The core service, not the Codex sidecar, reads these server environment settings:

- `OPENAI_API_KEY`: secret bearer credential, never serialized, logged or sent to Codex.
- `OPENAI_BASE_URL`: default `https://api.openai.com/v1`; must be an HTTP(S) base URL
  without embedded credentials, query or fragment. Append `/chat/completions`.
- `OPENAI_MODEL`: required vision-capable model name supported by that endpoint.
- `OPENAI_RESPONSE_FORMAT`: `json_schema` (default), `json_object`, or `none`.
- `OPENAI_MAX_OUTPUT_TOKENS`: default 8192, bounded to 256..32768.
- `OPENAI_TOKEN_PARAMETER`: `max_completion_tokens` (default) or `max_tokens` for
  providers that implement the older parameter.

Compose passes these explicitly to `core` from the ignored root `.env`; it never
passes them to `codex`. Invalid or incomplete configuration disables only this
provider, not manual editing or the subscription integration. Changes require
recreating only the core application service, never a workspace restart.

`GET /api/v1/ai/providers` returns:

```text
{providers: ProviderStatus[]}
ProviderStatus {
  id: "codex"|"openai-compatible", name: string, configured: boolean,
  model: string|null, endpoint: string|null, detail: string|null
}
```

The Codex entry describes the installed integration; login readiness still comes
from `/codex/status`. The API entry reports configuration, not a successful vendor
connection test. Its public endpoint must not contain credentials or query strings.

Add `provider: "codex"|"openai-compatible"` to AiRequest and AiRun. Missing provider
defaults to `codex` for already stored runs and existing callers. This selection is
frozen with the run and displayed in its history. Browser code never submits keys,
endpoint overrides or model overrides.

For the API provider, send standard Chat Completions messages with text context and
inline JPEG `image_url` parts. Do not send tools or execute returned tool calls.
Reject tool-call responses, oversized responses, non-JSON output and invalid proposals.
Parse into the same closed AiProposal and run the exact same scope/evidence/revision/
lock checks and atomic ChangeSet publication as Codex. Cancellation drops the HTTP
request and prevents publication; provider-side billing cancellation is best effort.
Keep errors actionable without exposing credentials or raw vendor responses.

The Agent workspace exposes an explicit provider selector. For the API provider,
show server-configured model/endpoint and `.env` instructions instead of a ChatGPT
login form. Consent refers to the selected provider, and the UI states that API
billing is separate from ChatGPT subscription usage. Both credential paths remain
unverified against a real model until the user supplies authorization or an API key.
