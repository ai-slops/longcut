# AI Editing

Longcut generates reviewable editing proposals through two explicitly selected
providers. Neither provider can apply edits without approval.

| Provider | Authentication | Billing |
| --- | --- | --- |
| Codex / ChatGPT | Official OpenAI device sign-in | Eligible ChatGPT plan's Codex usage and limits |
| OpenAI-compatible API | Server-side runtime configuration | The configured API provider's billing and limits |

There is no automatic fallback from the subscription path to the API path.
This deployment is a personal, single-user application. Restrict remote access to
trusted operators; do not expose a connected account or API key through a public,
unauthenticated shared service.

## Configure an API provider

Supply credentials through the runtime deployment environment or its secret mechanism.
For local Compose development, the ignored root `.env` can provide these values;
`.env.example` lists supported settings and contains no credentials. The three
required settings are:

```dotenv
OPENAI_API_BASE_URL=https://api.openai.com/v1
OPENAI_MODEL=your-vision-capable-model
OPENAI_API_KEY=your-private-key
```

Use a base URL, not the complete `/chat/completions` endpoint. The endpoint must
support the OpenAI-compatible Chat Completions interface and inline JPEG images.
Do not place credentials in the URL, a project brief, a prompt, or the browser.

Optional compatibility controls:

```dotenv
OPENAI_RESPONSE_FORMAT=json_schema
OPENAI_MAX_OUTPUT_TOKENS=8192
OPENAI_TOKEN_PARAMETER=max_completion_tokens
```

- `OPENAI_RESPONSE_FORMAT` accepts `json_schema`, `json_object`, or `none`.
  Regardless of vendor formatting support, Longcut parses and validates the same
  closed proposal contract before publication.
- `OPENAI_TOKEN_PARAMETER=max_tokens` supports providers requiring the older
  token-limit parameter. The configured limit must be between 256 and 32768.
- Streaming responses are bounded and parsed as SSE; an error event or tool-call
  response is never accepted as a successful edit.

After changing provider configuration, recreate only the agent runtime service.
Inside DIM:

```bash
docker compose up -d --no-deps --force-recreate agent-runtime
```

Do not restart the DIM workspace for an application configuration change. Never print
`docker compose config` when it may contain credentials; `docker compose config --quiet`
validates without displaying them. The legacy `OPENAI_BASE_URL` remains a compatibility
fallback, but `OPENAI_API_BASE_URL` is canonical and takes precedence when both are set.

## Connect a ChatGPT subscription

1. Open a project and select **Agent**.
2. Select **Codex / ChatGPT** as the provider.
3. Click **Connect ChatGPT**.
4. Open the displayed official OpenAI authorization URL and enter its one-time code.
5. Complete authentication in OpenAI's page. Longcut never asks for the password.
6. Wait for the connection to report **ready**.

Device-code login may need to be enabled in ChatGPT security settings. Subscription
availability and usage caps are controlled by OpenAI. Do not paste session tokens,
API keys or auth-file contents into Longcut or the project.

The Codex CLI is pinned to version 0.155.1. Credentials persist in the separate
`longcut-codex-auth` Docker volume; it is not mounted into the project core. The
shared agent-runtime service has no project database or original-media mount. Codex
children receive an explicit environment allowlist, and API credentials are not
passed to them. Model tools are restricted and any tool attempt invalidates the
proposal. A constrained Docker runtime may make tool execution entirely unavailable;
inference must then complete directly from supplied text and images without tools.

## Request an initial cut

1. Import a video and wait until its source is **ready**.
2. Open **Agent** and explicitly select the provider.
3. Choose **Source interval**, the source, and start/end frames.
4. Read the context disclosure, including the exact selected timing and sampled frames.
5. Enter an instruction, for example:
   - “Create two short scenes from this interval, preserving their original order.”
   - “Keep frames 300 through 900 and title the scene ‘First attempt’.”
6. Consent to sending the listed context to the selected provider, then request a proposal.
7. Track the run's actual sampling/generation status. Cancel if needed.
8. When ready, open **Review proposal**, select the operations to keep, and apply them.
9. Review playback and render a preview before accepting the result as finished.

Source-mode operations append proposed scenes; they do not replace existing edits.
Frame numbers use the project's 30 or 60 fps rate and half-open ranges: frame 300
is ten seconds at 30 fps, and `[300, 900)` is a twenty-second selection.

## Revise existing scenes

Choose **Existing scenes** and select the unlocked scenes to edit. Instructions
can request tighter in/out points, camera placement, scene title/purpose/caption
changes, or removal of selected scenes. For example:

- “Shorten these scenes while keeping their order and preserving the visible result.”
- “Move the existing camera to the upper-right corner and reduce its width.”
- “Change the selected scene caption to this exact sentence: …”

Requests never unlock scenes or mutate unrelated project settings. Manual edits
made after proposal creation cause property conflicts rather than silent overwrite.
A project revision change during generation makes the result stale; retry with the
new revision instead of silently applying an outdated plan.

## What the model actually receives

- Your instruction and the explicitly selected source/scene metadata.
- At most twelve timestamped, re-encoded JPEG samples from that selection.
- In scene mode, metadata for available camera sources needed for overlay choices.

Original video/audio files, the project database, API keys and Codex credentials
are not included. Sparse images cannot establish everything happening between
samples. This feature does **not** transcribe speech, listen to the soundtrack, or
infer a reliable story from an entire long broadcast. Review every proposed cut.
Local ASR is a separate pipeline; provider requests do not receive audio or infer
transcripts. Automatic transcript-grounded story editing remains outstanding.

## Troubleshooting

- **Configured** is not proof that the API key, model and vendor endpoint accept
  generation requests. A model-list response also does not prove generation access.
- **Invalid request / provider error:** check that the configured model supports
  images, the selected response format, and the token-limit parameter. The run
  remains failed and no changes are applied.
- **Usage limit:** check the selected provider's allowance or billing; Longcut does
  not switch providers automatically.
- **Stale:** the project changed during generation. Start a new request.
- **Locked or modified target:** retain the current edit or explicitly adjust the
  lock before requesting another proposal.
- **Codex tool attempt:** the proposal was discarded. Ask for a direct editing
  proposal from the supplied context, without filesystem or command execution.
- **Cancellation:** publication is prevented locally. A remote API provider may
  still bill work that it had already started.

Account authorization and real API generation are separate validation gates.
Mock-provider tests establish protocol and editing safeguards, not real vendor success.
