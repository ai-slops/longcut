FROM node:24.22.0-bookworm-slim@sha256:a5f4366e834695d5f363b920c104953a535d3a9864d1bdf74566a0ebe01daee5 AS base
RUN apt-get update && apt-get install -y --no-install-recommends bubblewrap ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && npm install --global @openai/codex@0.155.1 \
    && useradd --system --uid 10001 --create-home longcut \
    && mkdir -p /auth /run/codex-link /opt/longcut-agent-runtime \
    && chown -R longcut:longcut /auth /run/codex-link /opt/longcut-agent-runtime
ENV CODEX_HOME=/auth HOME=/home/longcut
USER longcut
WORKDIR /opt/longcut-agent-runtime

FROM base AS runtime
COPY --chown=longcut:longcut package.json pnpm-lock.yaml ./
RUN npx --yes pnpm@10.34.7 install --prod --frozen-lockfile
COPY --chown=longcut:longcut src ./src
EXPOSE 4100
CMD ["node", "src/server.ts"]
