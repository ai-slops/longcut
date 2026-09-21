#!/usr/bin/env sh
set -eu

# DIM supplies a read-only manifest path to Project lifecycle hooks.
# Forward only its approved Git address; do not discover or guess host routes.
: "${DIM_PROJECT_MANIFEST:?DIM_PROJECT_MANIFEST is required}"
: "${DIM_GIT_BASE_URL:?DIM_GIT_BASE_URL is required}"
test -r "$DIM_PROJECT_MANIFEST"

jq -e --arg root "${DIM_PROJECT_ROOT:-$PWD}" --arg git "$DIM_GIT_BASE_URL" '
  if .schemaVersion != 2 or .root.path != $root or .gitBaseUrl != $git then
    error("DIM manifest does not match this project lifecycle")
  elif (.hostAliases["dim-gitea"] | type) != "array" then
    error("DIM manifest has no approved dim-gitea address list")
  elif (.hostAliases["dim-gitea"] | length) == 0 then
    error("DIM manifest has no approved dim-gitea addresses")
  elif all(.hostAliases["dim-gitea"][]; type == "string" and length > 0) | not then
    error("DIM Git addresses must be nonempty strings")
  else
    {services:{agent:{extra_hosts:[.hostAliases["dim-gitea"][] | "dim-gitea=\(.)"]}}}
  end
' "$DIM_PROJECT_MANIFEST"
