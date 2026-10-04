import { AlertTriangle, CheckCircle2, Link2, XCircle } from "lucide-react"
import type { SourceSessionSummary } from "../session-domain"
import { sourceSessionAssetUrl } from "../session-links"
import { Badge } from "./Ui"

function durationLabel(durationNs: string): string {
  const totalSeconds = BigInt(durationNs) / 1_000_000_000n
  const hours = totalSeconds / 3600n
  const minutes = (totalSeconds % 3600n) / 60n
  const seconds = totalSeconds % 60n
  return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`
}

export function SessionSummary({
  projectId,
  session,
  assetsDownloadable,
}: {
  readonly projectId: string
  readonly session: SourceSessionSummary
  readonly assetsDownloadable: boolean
}) {
  return (
    <div className="session-summary">
      <header>
        <div>
          <strong>{session.title}</strong>
          <small>
            {session.session_id} · {durationLabel(session.duration_ns)}
          </small>
        </div>
        <Badge
          tone={
            session.conformance === "conformant"
              ? "success"
              : session.conformance === "rejected"
                ? "error"
                : "warning"
          }
        >
          {session.conformance}
        </Badge>
      </header>
      <div className="session-summary__sources">
        {session.sources.map((source) => (
          <div className="session-summary__source" key={source.source_id}>
            {source.editor_source_id ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
            <span>
              <strong>{source.display_name}</strong>
              <small>
                {source.semantic_role} · {source.kind}
              </small>
            </span>
            <Badge tone={source.editor_source_id ? "success" : "error"}>
              {source.editor_source_id ? "editable" : "preserved / unsupported"}
            </Badge>
            {source.reason ? <p>{source.reason}</p> : null}
          </div>
        ))}
      </div>
      <ul className="session-summary__assets" aria-label="Package assets">
        {session.assets.map((asset) => {
          const href = sourceSessionAssetUrl(
            projectId,
            session.id,
            asset.asset_id,
            assetsDownloadable,
          )
          return (
            <li key={asset.asset_id}>
              {asset.availability === "online" && href ? (
                <a className="session-summary__asset" href={href} download>
                  <span>{asset.uri}</span>
                  <Badge tone="success">online</Badge>
                </a>
              ) : (
                <div className="session-summary__asset">
                  <span>{asset.uri}</span>
                  <Badge tone={asset.availability === "online" ? "neutral" : "error"}>
                    {asset.availability === "online" ? "staged" : "offline"}
                  </Badge>
                </div>
              )}
            </li>
          )
        })}
      </ul>
      {session.assets.some((asset) => asset.availability === "offline") ? (
        <div className="session-summary__notice">
          <AlertTriangle size={14} /> Some package assets are offline.
        </div>
      ) : null}
      {session.warnings.length > 0 ? (
        <ul>
          {session.warnings.map((warning) => (
            <li key={`${warning.code}:${warning.path}`}>
              <strong>{warning.code}</strong> · {warning.path}
              <br />
              {warning.message}
            </li>
          ))}
        </ul>
      ) : null}
      {session.verification_url ? (
        <a href={session.verification_url}>
          <Link2 size={13} /> Verification record
        </a>
      ) : null}
    </div>
  )
}
