# Router4u for live Longcut testing

## Installation in this DIM workspace

- Control room: <https://longcut--3.remote-main-2.mj-y.com/>
- External OpenAI-compatible base URL: `https://longcut--3.remote-main-2.mj-y.com/v1`
- Private base URL from this workspace or its Docker containers:
  `http://agent-dind:9470/v1`
- Container: `longcut-router4u`, image: `longcut-router4u:a6709ad`.
- Upstream revision: `a6709ad6e892d4b26bd7d7cf611a76b29436c0a0`.
- Checkout: `/tmp/opencode/router4u` (temporary; not persistent service state).
- Persistent volumes: `longcut-router4u-data` and `longcut-router4u-transcripts`.
  Both contain sensitive information. Do not export, commit, or delete them.

The image uses upstream's pinned CLIProxyAPI release and checksum. This Docker
installation lacks BuildKit; only `COPY --chmod=0555` was replaced with equivalent
`COPY` plus `RUN chmod 0555` before switching to the non-root user.
The container uses a read-only root, writable `/tmp`, bounded logs, and automatic
restart. Only gateway port 9470 is published on the private DIM Docker host;
backend port 8317 is not published.

DIM ingress reaches the workspace, not the remote Docker daemon. The byte-preserving
relay `tools/router4u-forward.mjs` forwards workspace port 9470 to `agent-dind:9470`,
retaining Host, Origin, cookies, and streaming. After a workspace restart, restart
the relay if necessary, then register the ingress again:

```sh
node tools/router4u-forward.mjs
# In a separate terminal:
dim-development-service expose --name router4u --port 9470 --ingress https-ts --require-scheme https
```

The relay is currently running as a detached process; it is not a durable workspace
startup hook. Docker service restart and workspace relay restart are separate.

## User-owned login and key issuance

Retrieve the operator key yourself in the workspace terminal, not in chat:

```sh
docker exec longcut-router4u router4u admin-key show
```

1. Open the control-room URL and enter that key. Do not use it as an API key.
2. Choose **Connections → personal → Connect Codex** and complete account login.
3. If redirected to an unreachable `localhost:1455`, paste the complete callback
   URL into Router4u's callback dialog. Do not share that URL in chat or expose 1455.
4. Wait for confirmed login, then **Set up models → Load available models**.
   Select the model to test and choose **Use selected models**.
5. Set **Request routing** to **On**, then save.
6. In **Projects & keys**, create a Longcut project, authorize `personal`, set
   appropriate limits, and issue a dedicated application/project key.

Use only your own account and applications, subject to provider terms and limits.
Router4u records sensitive prompt/response exchanges in its transcript volume.

## Longcut configuration and live-test boundary

Configure the PART runtime, not the Rust core, with:

```text
OPENAI_BASE_URL=http://agent-dind:9470/v1
OPENAI_MODEL=<exact model ID selected in Router4u>
OPENAI_API_KEY=<dedicated project key, supplied through private process configuration>
```

Use the external base URL when testing from outside DIM. Do not write a key into
this document, source control, or nonsecret JSON. Existing fixture services on
ports 3020/4020/4110 were deliberately left unchanged; they are not live providers.
The key and selected model are required before creating a separate live Longcut
runtime. See `docs/agent-runtime.md` for runtime configuration.

First list `/v1/models` with the project bearer key, without inference. Then use a
synthetic source and an explicit Longcut consent request to test Chat Completions,
JPEG input, and structured proposal output. Inspect Router4u's request record and
the actual PART execution ID. A completed response must become a Ready ChangeSet
without editing scenes until explicit approval. Health and model listing alone do
not establish model or structured-output compatibility.

## Verified readiness

- Docker build succeeded; container reports `running / healthy`.
- `router4u health`: local ready, management reachable, pinned backend version
  matches; authentication missing and provider not probed, as expected before login.
- External HTTPS returns 200 and a real browser renders the operator-key login.
- Unauthenticated `/v1/models` rejects access with `authentication_failed`.
- Relay passes Node syntax validation. LSP unavailable in this workspace.
- No operator/project key or OAuth credential was read; no real inference sent.

To stop without deleting data: `docker stop -t 30 longcut-router4u`.
To restart: `docker start longcut-router4u`. Never remove the two named volumes as
part of routine restart or upgrade.
