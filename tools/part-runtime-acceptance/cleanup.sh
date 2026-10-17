#!/usr/bin/env bash
set -euo pipefail

PID_FILE=/tmp/part-runtime-acceptance-gateway.pid
if [[ -f "$PID_FILE" ]]; then
  PID=$(<"$PID_FILE")
  if [[ -r "/proc/$PID/cmdline" ]]; then
    COMMAND=$(tr '\0' ' ' <"/proc/$PID/cmdline")
    if [[ "$COMMAND" == *node@24.22.0* ]]; then
      kill -- "-$PID" 2>/dev/null || true
    fi
  fi
  rm -f "$PID_FILE"
fi
docker rm -f part-runtime-acceptance-core part-runtime-acceptance-runtime >/dev/null 2>&1 || true
docker volume rm part-runtime-acceptance-data part-runtime-acceptance-link >/dev/null 2>&1 || true
docker network rm part-runtime-acceptance >/dev/null 2>&1 || true
