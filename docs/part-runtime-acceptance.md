# PART runtime acceptance

This acceptance stack proves the real `@slop-lab/part@0.1.0` runtime and the
latest Docker-built Rust core without contacting a paid or external model. Both
providers are explicitly marked `test_provider=true`. The fake Codex login
status is protocol input only and is not evidence of a ChatGPT login.

## Run

```bash
tools/part-runtime-acceptance/run.sh
```

The launcher uses Node 24.22 through `npx node@24.22.0`, builds the frontend,
builds explicit Docker tar contexts containing no `.env` files, starts the
owned stack, runs all acceptance scenarios, and leaves the stack running.

The current machine-readable and human-readable results are:

- `tools/part-runtime-acceptance/evidence/proof.json`
- `tools/part-runtime-acceptance/evidence/proof.md`

The proof records real PART execution UUIDs, profile revisions, local-provider
flags, sampled source frame/evidence IDs, provider request inspections, project
scene snapshots before and after approval, cancellation cleanup, stale-result
rejection, runtime/Node versions, and live URLs. It intentionally records only
that fixture bearer authentication was valid, never the authorization value.

## Live stack

- Browser and gateway: <http://127.0.0.1:3020>
- Rust core from the workspace: <http://agent-dind:4020>
- Public runtime health: <http://agent-dind:4110/health>
- Fixture control: <http://agent-dind:4111>

The launcher leaves `PART Codex ready browser review` with a non-empty Ready
ChangeSet and unchanged scenes. During subsequent browser QA, the Codex proposal
was explicitly applied (scenes 0 to 1, revision 2 to 3), then another compatible
proposal was requested through the consent form. That proposal reached Ready
without changing the one existing scene. All jobs are terminal.

## Final browser verification

- Actual Codex PART execution: `b3aa56d0-6c59-4e2f-888c-f420688e3ab2`.
- Subsequent compatible execution: `532b1664-4734-4156-8996-0903ec282222`.
- Fresh captures: `artifacts/part-runtime-live-{1280,768,375}-{top,details}.png`.
  Top and expanded-details captures are distinct and show both provider identities.
- Mobile scope controls measure 44 CSS pixels tall; at 375 pixels the document
  width equals the viewport width. Independent final visual review passed.
- Final web tests: 97 passed; TypeScript check and production build passed.
- Runtime service: 36 tests passed. Docker Rust workspace formatting, clippy
  with warnings denied, and tests passed before the final CSS-only adjustment.
- CSS LSP was unavailable (installation previously declined); production build
  and browser measurement were used instead. Zod dependency annotation warnings
  remain non-fatal during bundling.

These are real PART executions with local model fixtures, not evidence of paid
API inference or successful authentication to a real ChatGPT account.

## Coverage

- OpenAI-compatible JSON/SSE transport checks bearer auth, strict response
  format, actual JPEG inputs, and Rust sampling evidence.
- Fake Codex checks production policy argv, schema path, actual JPEG files, and
  closed proposal output while using production process-group handling.
- Both providers pass through the installed PART agent and expose the actual
  PART `handle.ref.id` through Rust's public runtime projection.
- Compatible-provider edits remain drafts until an explicit
  `apply_change_set`; browser QA also explicitly applied the Codex proposal.
- Public cancellation reaches private runtime phase `terminal`, closes the held
  fixture request, and publishes no ChangeSet.
- A held response released after a manual revision becomes `stale`, leaves
  scenes unchanged, and publishes no ChangeSet.

## Cleanup

Cleanup is deliberately scoped to resources owned by this tooling:

```bash
tools/part-runtime-acceptance/cleanup.sh
```

It stops only the recorded gateway process group and the
`part-runtime-acceptance-*` containers, volumes, and network. It does not touch
the QA server on port 3011 or any production credential volume.
