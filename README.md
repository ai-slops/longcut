# Longcut

A local-first editing workspace for turning long recordings into reviewed cuts.
The first implementation follows the import, manual editing, camera composition,
feedback, ChangeSet, and render sequence from the supplied product specification.

**Status: validated initial vertical slice, not Stage 1 alpha.** The full
roadmap includes Korean transcription, semantic moment/story analysis, OBS capture,
long-session acceptance, and more advanced timeline operations. Those capabilities
must not be inferred from the existence of the initial editor.

See [validation evidence and remaining work](docs/validation.md) for the exact
tested behaviors, known limitations, and next implementation packages.

## Architecture

- **Rust / Axum:** project API, command validation, media jobs and immutable renders.
- **SQLite WAL:** committed project state, command journal and undo/redo history.
- **FFmpeg / ffprobe:** real original-media imports, derivatives and H.264/AAC output.
- **React / TypeScript / Vite:** source, story, edit, review and deliverable workspaces.
- **Node 24 gateway:** same-origin static UI and streaming API/media proxy.
- **Persistent Docker volume:** originals, derived files and database survive restarts.

See [the implementation contract](docs/implementation-contract.md) for API shapes,
time rules and the exact first-increment scope. [DESIGN.md](DESIGN.md) records UI tokens
and accessibility decisions. New project documentation is written in English.

## Run inside this DIM workspace

The agent has Node 24 and a private remote Docker daemon at `agent-dind`. Host-side
bind mounts do not refer to the agent checkout. Docker build contexts transfer the
source; the named volume stores project data inside the daemon.

```bash
# Install and build the frontend.
npx --yes pnpm@10 --dir apps/web install --frozen-lockfile
npx --yes pnpm@10 --dir apps/web build

# Only the private agent/DinD network can reach this published backend port.
LONGCUT_CORE_BIND=0.0.0.0 docker compose up -d --build

# Use another port if the initial DIM service still occupies 3000.
PORT=3001 LONGCUT_BACKEND=http://agent-dind:4000 node app/server.mjs
```

From a second terminal, register the running gateway with DIM:

```bash
dim-development-service expose --name longcut --port 3001 \
  --ingress https-ts --require-scheme https
```

Use the printed URL. This does not expose a host Docker socket or change the trusted
`.dim/` lifecycle. `.dim/agent/start.sh` continues to run `node app/server.mjs` on 3000
when the workspace service starts; the compiled frontend must already be present.

## Run on a local development machine

Install Node 24 and Docker Compose. Use the same frontend commands, then:

```bash
docker compose up -d --build
LONGCUT_BACKEND=http://127.0.0.1:4000 node app/server.mjs
```

Open <http://127.0.0.1:3000>. The backend defaults to a loopback-only published port.
Do not publish it to an untrusted network: this is a single-user local application,
not an authenticated multi-user service. Use trusted DIM ingress for remote testing.

For frontend hot reload, use `npx --yes pnpm@10 --dir apps/web dev`. Its API proxy
targets the local backend on port 4000; use the built gateway path inside DIM.

## First editing session

1. Create a project with a 30 or 60 fps output rate.
2. Import a recording, or generate the clearly marked synthetic sample.
3. Select source In/Out points and append three scenes to the program.
4. Select a scene to trim, split, reorder, rename or add a caption.
5. Import a second recording as the camera and adjust its scene composition.
6. Add a scene-local range comment and optional frame pin.
7. Review explicit typed ChangeSet operations; approve only the intended operations.
8. Request an exact preview or final render. Download the video, subtitles and lineage.

Browser composition is approximate. An exact render is immutable and pinned to the
revision that was queued; editing afterward does not rewrite that rendered version.
Typed proposal review is not a claim that an LLM or ASR provider has been configured.

## Validation

```bash
node --test app/gateway.test.mjs
npx --yes pnpm@10 --dir apps/web test
npx --yes pnpm@10 --dir apps/web typecheck
npx --yes pnpm@10 --dir apps/web build
docker build --target check -t longcut-check .

# Creates an isolated synthetic project through the running, real API.
LONGCUT_URL=http://127.0.0.1:3001 node tools/acceptance.mjs
```

The acceptance driver covers multipart import, proxy byte ranges, three source
spans, trim/split/reorder, undo/redo, locks, stale revisions, comments, partial
proposal approval, idempotency, frozen final render and downloadable output files.
It does not delete user projects. Long-duration, VFR and real broadcast validation
remain separate from synthetic acceptance.

## Data safety and operations

- Original uploaded files are managed copies, not modified in place.
- The `longcut-projects` volume holds all projects and artifacts.
- `docker compose stop` preserves data. **Do not run `docker compose down -v` unless
  you intend to delete project data.**
- Stop the core before taking a filesystem-level volume backup, so SQLite and
  artifact metadata are consistent. Keep a second copy of original recordings.
- Use `docker compose logs core` for structured worker/server errors.
- The gateway reports a useful error when the core is unreachable.
- The public GitLab DIM link required authentication in this environment. The
  installed proxy documentation points to the official
  [dev-infra-manager repository](https://github.com/slop-lab/dev-infra-manager).
