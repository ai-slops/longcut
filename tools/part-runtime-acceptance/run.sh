#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"

./tools/part-runtime-acceptance/cleanup.sh
npx --yes node@24.22.0 "$(command -v npx)" --yes pnpm@10.34.7 --dir apps/web build

tar --exclude='*.env' --exclude='.env*' -cf - \
  tools/part-runtime-acceptance/Dockerfile \
  tools/part-runtime-acceptance/fixture-contract.mjs \
  tools/part-runtime-acceptance/fake-codex.mjs \
  tools/part-runtime-acceptance/stack.mjs \
  services/agent-runtime/package.json \
  services/agent-runtime/pnpm-lock.yaml \
  services/agent-runtime/src \
  | docker build -f tools/part-runtime-acceptance/Dockerfile -t part-runtime-acceptance-runtime:latest -

tar --exclude='*.env' --exclude='.env*' -cf - \
  tools/part-runtime-acceptance/core.Dockerfile \
  Cargo.toml Cargo.lock crates \
  | docker build -f tools/part-runtime-acceptance/core.Dockerfile -t part-runtime-acceptance-core:latest -

docker network create part-runtime-acceptance >/dev/null
docker volume create part-runtime-acceptance-link >/dev/null
docker volume create part-runtime-acceptance-data >/dev/null
docker run --rm --user root -v part-runtime-acceptance-data:/data part-runtime-acceptance-core:latest \
  sh -c 'touch /data/.acceptance-owned && chown 10001:999 /data /data/.acceptance-owned'
docker run -d --name part-runtime-acceptance-runtime \
  --network part-runtime-acceptance \
  -p 4110:4110 -p 4111:4111 \
  -v part-runtime-acceptance-link:/run/codex-link \
  part-runtime-acceptance-runtime:latest >/dev/null

npx --yes node@24.22.0 -e 'for(let i=0;i<120;i++){try{const r=await fetch("http://agent-dind:4110/health");if(r.ok)process.exit(0)}catch{}await new Promise(r=>setTimeout(r,500))}process.exit(1)'
docker run -d --name part-runtime-acceptance-core \
  --network part-runtime-acceptance \
  -p 4020:4000 \
  -e LONGCUT_AGENT_RUNTIME_URL=http://part-runtime-acceptance-runtime:4110 \
  -e LONGCUT_AGENT_RUNTIME_TOKEN_FILE=/run/codex-link/token \
  -v part-runtime-acceptance-link:/run/codex-link:ro \
  -v part-runtime-acceptance-data:/data \
  part-runtime-acceptance-core:latest >/dev/null

npx --yes node@24.22.0 -e 'for(let i=0;i<120;i++){try{const r=await fetch("http://agent-dind:4020/api/v1/health");if(r.ok)process.exit(0)}catch{}await new Promise(r=>setTimeout(r,500))}process.exit(1)'
setsid env PORT=3020 LONGCUT_GATEWAY_BIND=127.0.0.1 LONGCUT_BACKEND=http://agent-dind:4020 \
  npx --yes node@24.22.0 app/server.mjs >/tmp/part-runtime-acceptance-gateway.log 2>&1 &
echo $! >/tmp/part-runtime-acceptance-gateway.pid
npx --yes node@24.22.0 -e 'for(let i=0;i<120;i++){try{const r=await fetch("http://127.0.0.1:3020/api/v1/health");if(r.ok)process.exit(0)}catch{}await new Promise(r=>setTimeout(r,250))}process.exit(1)'

LONGCUT_URL=http://127.0.0.1:3020 PART_FIXTURE_URL=http://agent-dind:4111 \
  npx --yes node@24.22.0 tools/part-runtime-acceptance/acceptance.mjs
