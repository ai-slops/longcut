export function sourceSessionAssetUrl(
  projectId: string,
  sessionId: string,
  assetId: string,
  active: boolean,
): string | null {
  if (!active) return null
  return `/api/v1/projects/${encodeURIComponent(projectId)}/source-sessions/${encodeURIComponent(sessionId)}/assets/${encodeURIComponent(assetId)}`
}
