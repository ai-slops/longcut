FROM node:24.19.0-bookworm-slim@sha256:a9f5f7c91a432850b2a8a7797adf5eadb6c733ceed61167806cee7ea7fbc29df AS base
RUN apt-get update && apt-get install -y --no-install-recommends bubblewrap ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && npm install --global @openai/codex@0.155.1 \
    && useradd --system --uid 10001 --create-home longcut \
    && mkdir -p /auth /run/codex-link /opt/longcut-codex \
    && chown -R longcut:longcut /auth /run/codex-link /opt/longcut-codex
ENV CODEX_HOME=/auth HOME=/home/longcut
USER longcut
WORKDIR /opt/longcut-codex

FROM base AS runtime
COPY --chown=longcut:longcut package.json pnpm-lock.yaml ./
RUN npx --yes pnpm@10.34.5 install --prod --frozen-lockfile
COPY --chown=longcut:longcut src ./src
EXPOSE 4100
CMD ["node", "src/server.ts"]
