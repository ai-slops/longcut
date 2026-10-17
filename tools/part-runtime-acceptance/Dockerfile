FROM node:24.22.0-bookworm-slim@sha256:a5f4366e834695d5f363b920c104953a535d3a9864d1bdf74566a0ebe01daee5
RUN useradd --system --uid 10001 --create-home longcut \
    && mkdir -p /opt/longcut-agent-runtime /acceptance /acceptance/state /run/codex-link \
    && chown -R longcut:longcut /opt/longcut-agent-runtime /acceptance /run/codex-link
WORKDIR /opt/longcut-agent-runtime
COPY --chown=longcut:longcut services/agent-runtime/package.json services/agent-runtime/pnpm-lock.yaml ./
RUN npx --yes pnpm@10.34.7 install --prod --frozen-lockfile
COPY --chown=longcut:longcut services/agent-runtime/src ./src
COPY --chown=longcut:longcut tools/part-runtime-acceptance/*.mjs /acceptance/
USER longcut
CMD ["node", "/acceptance/stack.mjs"]
