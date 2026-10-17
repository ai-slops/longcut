# PART-backed agent runtime

Longcut's two proposal providers now execute through a shared PART service:

```text
Browser consent and selected scope
  -> Rust: persisted AI run, frozen context, JPEG sampling
  -> private AgentRuntimeClient
  -> PART agent -> model gateway -> provider adapter/transport
  -> validated proposal
  -> Rust: scope/evidence/stale-revision checks -> Ready ChangeSet
  -> explicit human approval -> project commands
```

Pinned dependencies are `@slop-lab/part@0.1.0` and
`@slop-lab/part-plugins@0.1.0`, based on upstream commit
[`c85038744bfef810551984e3e9da4d447839caab`](https://github.com/slop-lab/part/tree/c85038744bfef810551984e3e9da4d447839caab).
The runtime requires **Node 24.22.0 or newer**. The Docker image pins Node 24.22.0.

## What PART owns

Each generation calls `createAgent().start()`, then PART's context/model gateway,
auth binding, transport and decoder. The actual execution ID is exposed separately
from Longcut's persisted AI run ID. A validating driver accepts only a complete,
tool-free response matching the existing closed Proposal schema.

There is no model-advertised editing, shell, filesystem or network tool. Limits allow
one model invocation and one executor attempt, with zero tool dispatches. This is
proposal-only runtime orchestration, not an autonomous editor or general coding agent.

The upstream bundled provider adapters do not support Longcut's image input or
Codex subscription login. Longcut therefore supplies two explicit extensions:

- **Vision Chat Completions adapter:** preserves JPEG samples, structured-output
  schema, token-parameter options and JSON/SSE responses; uses PART's fetch transport
  with origin pinning, redirect rejection and application output bounds.
- **Codex broker adapter/transport:** retains official device login, pinned Codex
  0.155.1, sandbox policy, event gate, private temporary files and process-group cleanup.
  Codex account management is application-owned, not native PART OAuth support.

Both generation paths now traverse PART. Rust's direct external OpenAI HTTP executor
has been removed; Rust still owns durable project/run state and authorization.

## Deployment and migration

`services/agent-runtime` replaces `services/codex`. `compose.yaml` now starts
`agent-runtime` on the unchanged private port **4100** and retains the `codex`
network alias plus the named `longcut-codex-auth` and `longcut-codex-link` volumes.
Project media and SQLite storage remain in `longcut-projects`.

Do not run old and new account services simultaneously against the same auth volume.
Before upgrading an existing Compose project, inspect its containers and stop the
old Codex service using the prior Compose configuration. Rebuild the core and runtime
together. If using `--remove-orphans`, first verify that every orphan belongs to this
upgrade; it removes containers, not the named account/project volumes. Never use
`docker compose down -v` as an upgrade step.

Core connection settings:

| Current name | Compatibility fallback |
| --- | --- |
| `LONGCUT_AGENT_RUNTIME_URL` | `LONGCUT_CODEX_URL` |
| `LONGCUT_AGENT_RUNTIME_TOKEN_FILE` | `LONGCUT_CODEX_TOKEN_FILE` |

OpenAI-compatible configuration now belongs to the **runtime service process**, not
the Rust core: `OPENAI_API_KEY`, `OPENAI_API_BASE_URL`, `OPENAI_MODEL`,
`OPENAI_RESPONSE_FORMAT`, `OPENAI_MAX_OUTPUT_TOKENS`, and `OPENAI_TOKEN_PARAMETER`.
The legacy `OPENAI_BASE_URL` remains supported as a fallback; when both base URL
variables are set, `OPENAI_API_BASE_URL` takes precedence. Missing configuration rejects
a compatible request before it is persisted or sampled.

Supply credentials through the deployment environment or its secret mechanism.
Nonsecret configuration belongs in ordinary configuration files, not `*.env` files.
The migration tooling does not inspect environment files or copy existing credentials.

## Lifecycle and trust boundaries

- Private routes require the shared Bearer link token; only health is public.
- Codex children receive an explicit environment allowlist, never inherited API keys.
- Codex safety failure disables Codex rather than weakening its sandbox. Compatible
  provider generation and manual editing remain usable independently.
- Runtime jobs are bounded, in-memory execution records. Rust SQLite remains the
  durable run/ChangeSet record; runtime restart/eviction can remove observational
  details without removing a published proposal.
- Explicit cancellation requests PART cancellation and awaits owned fetch/process
  cleanup beyond PART's default cancellation grace. Concurrent cancel calls share
  the same completion barrier. Terminal success/failure is not published early.
- Rust owns attempted remote jobs through transport/decode/poll failure and dropped
  futures. Cleanup always targets the requested job ID, never an untrusted response ID.
- Engine/version/protocol, job/execution identity, provider profile, monotonic progress
  and terminal response shape are validated before accepting runtime results.
- Progress is a fixed set of **host-authored lifecycle phases**, not raw PART/provider
  text-delta streaming. Prompts, model bodies, credentials and subprocess logs are not
  forwarded to the browser.
- The runtime process contains both provider integrations. Although Codex child
  isolation is preserved, compromise of the trusted runtime process has a larger
  credential blast radius than separate provider hosts. PART capability declarations
  are not an OS sandbox.

Public AI request/run JSON, provider IDs and omitted-provider Codex default remain
compatible. Existing Codex account routes remain. New availability/observation
resources are described in [the protocol contract](part-runtime-contract.md).
Runtime availability is deliberately separate from provider configuration and login.

## Validation and use

With Node 24.22+:

```bash
npx --yes pnpm@10 --dir services/agent-runtime install --frozen-lockfile
npx --yes pnpm@10 --dir services/agent-runtime typecheck
npx --yes pnpm@10 --dir services/agent-runtime test
```

Runtime tests execute real PART against local HTTP fixtures and a fake Codex binary.
They cover multimodal/schema compatibility, auth isolation, output/redirect/tool
rejection, UTF-8/SSE failures, cancellation races, cleanup and shutdown.

The reusable [full-stack acceptance driver](part-runtime-acceptance.md) runs the real
Rust core and PART runtime together. Both provider fixtures return nonempty proposals;
the driver proves that scenes do not change until explicit approval, cancellation
publishes nothing, and stale revisions discard proposals. Fixture mode is clearly
labelled and performs no external model requests or real ChatGPT login.

The lead also used the browser to inspect the actual PART execution ID and manually
apply the Codex fixture's Ready ChangeSet: scene count changed from zero to one only
after selecting an operation and pressing **Apply selected**. This is integration
evidence, not a claim that a production account or paid model has been exercised.
