import { ExternalLink, History } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { ApiError, api } from "../api"
import type { Story } from "../domain"
import { StoryVersionRequestCoordinator, storyVersionIdentity } from "../story-version-request"
import { Badge, Button } from "./Ui"

type StoryVersionLinkProps = {
  readonly projectId: string
  readonly version: string
}

export function StoryVersionLink({ projectId, version }: StoryVersionLinkProps) {
  const identity = storyVersionIdentity(projectId, version)
  const coordinator = useRef(new StoryVersionRequestCoordinator())
  const [loaded, setLoaded] = useState<{ readonly key: string; readonly snapshot: Story } | null>(
    null,
  )
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const snapshot = loaded?.key === identity.key ? loaded.snapshot : null

  useEffect(() => {
    coordinator.current.invalidate()
    setLoaded((current) => (current?.key === identity.key ? current : null))
    setBusy(false)
    setError(null)
    return () => coordinator.current.invalidate()
  }, [identity.key])

  async function load(): Promise<void> {
    const ticket = coordinator.current.begin(identity)
    setBusy(true)
    setError(null)
    try {
      const incoming = await api.getStoryVersion(ticket.identity, ticket.signal)
      if (coordinator.current.owns(ticket)) setLoaded({ key: identity.key, snapshot: incoming })
    } catch (reason) {
      if (!coordinator.current.owns(ticket)) return
      setError(
        reason instanceof ApiError || reason instanceof Error
          ? reason.message
          : "Version request failed.",
      )
    } finally {
      if (coordinator.current.release(ticket)) setBusy(false)
    }
  }

  return (
    <div className="story-version">
      <div className="cluster">
        <Button type="button" variant="ghost" busy={busy} onClick={() => void load()}>
          <History size={14} /> Preview outline v{version}
        </Button>
        <a
          href={`/api/v1/projects/${projectId}/story/versions/${version}`}
          target="_blank"
          rel="noreferrer"
        >
          JSON <ExternalLink size={12} />
        </a>
      </div>
      {error ? (
        <p className="error-copy" role="alert">
          {error}
        </p>
      ) : null}
      {snapshot ? (
        <div className="story-version__snapshot">
          <Badge tone="violet">Story v{snapshot.version}</Badge>
          <span>{snapshot.chapters.length} chapters</span>
          <span>
            {snapshot.chapters.reduce((count, chapter) => count + chapter.beats.length, 0)} beats
          </span>
        </div>
      ) : null}
    </div>
  )
}
