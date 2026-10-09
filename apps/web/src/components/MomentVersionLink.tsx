import { ExternalLink, History } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { ApiError, api } from "../api"
import type { Moment } from "../domain"
import {
  MomentVersionRequestCoordinator,
  momentVersionIdentity,
  snapshotMatchesIdentity,
} from "../moment-version-request"
import { Badge, Button } from "./Ui"

type MomentVersionLinkProps = {
  readonly projectId: string
  readonly momentId: string
  readonly version: string
}

export function MomentVersionLink({ projectId, momentId, version }: MomentVersionLinkProps) {
  const identity = momentVersionIdentity(projectId, momentId, version)
  const coordinator = useRef(new MomentVersionRequestCoordinator())
  const [loaded, setLoaded] = useState<{
    readonly identityKey: string
    readonly snapshot: Moment
  } | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const path = `/api/v1/projects/${projectId}/moments/${momentId}/versions/${version}`
  const snapshot = loaded?.identityKey === identity.key ? loaded.snapshot : null
  const identityKey = identity.key

  useEffect(() => {
    coordinator.current.invalidate()
    setLoaded((current) => (current?.identityKey === identityKey ? current : null))
    setBusy(false)
    setError(null)
    return () => coordinator.current.invalidate()
  }, [identityKey])

  async function load(): Promise<void> {
    const ticket = coordinator.current.begin(identity)
    setBusy(true)
    setError(null)
    try {
      const incoming = await api.getMomentVersion({ ...identity, signal: ticket.signal })
      if (!coordinator.current.owns(ticket)) return
      if (!snapshotMatchesIdentity(identity, incoming)) {
        setError("Version response identity mismatch.")
        return
      }
      setLoaded({ identityKey: identity.key, snapshot: incoming })
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
    <div className="moment-version">
      <div className="cluster">
        <Button type="button" variant="ghost" busy={busy} onClick={() => void load()}>
          <History size={14} /> Preview frozen v{version}
        </Button>
        <a href={path} target="_blank" rel="noreferrer">
          JSON <ExternalLink size={12} />
        </a>
      </div>
      {error ? (
        <p className="error-copy" role="alert">
          {error}
        </p>
      ) : null}
      {snapshot ? (
        <div className="moment-version__snapshot">
          <div className="cluster">
            <strong>{snapshot.label}</strong>
            <Badge tone="violet">Moment v{snapshot.version}</Badge>
          </div>
          <p className="moment-version__range">
            {snapshot.source_id} · frames {snapshot.start_frame}–{snapshot.end_frame}
          </p>
          <div className="cluster">
            <Badge>{snapshot.review_status}</Badge>
            {snapshot.archived ? <Badge tone="warning">Archived</Badge> : null}
            <Badge tone="violet">{snapshot.narrative_role}</Badge>
          </div>
          {snapshot.note ? <p className="moment-version__note">{snapshot.note}</p> : null}
          {snapshot.evidence.map((item) => (
            <figure key={`${item.transcript_id}:${item.start_frame}`}>
              <blockquote>{item.quote}</blockquote>
              <figcaption>{item.origin} · frozen transcript evidence</figcaption>
            </figure>
          ))}
          <dl className="moment-version__provenance">
            <div className="moment-version__provenance-row">
              <dt>Created</dt>
              <dd>
                revision {snapshot.provenance.created_revision} · command{" "}
                {snapshot.provenance.created_command_id} · {snapshot.provenance.created_at}
              </dd>
            </div>
            <div className="moment-version__provenance-row">
              <dt>Updated</dt>
              <dd>
                revision {snapshot.provenance.updated_revision} · command{" "}
                {snapshot.provenance.updated_command_id} · {snapshot.provenance.updated_at}
              </dd>
            </div>
          </dl>
        </div>
      ) : null}
    </div>
  )
}
