import { Activity, ChevronDown } from "lucide-react"
import { useState } from "react"
import { ApiError, api } from "../api"
import type { SourceSessionSummary } from "../session-domain"
import {
  diagnosticSegmentRows,
  paginateDiagnostics,
  type SyncDiagnostics,
  unsupportedMappingRows,
} from "../sync-diagnostics-domain"
import { SyncDiagnosticRanges } from "./SyncDiagnosticRanges"
import { Badge, Button } from "./Ui"

const PAGE_SIZE = 40

type SyncDiagnosticsPanelProps = {
  readonly projectId: string
  readonly session: SourceSessionSummary
}

function errorMessage(reason: unknown): string {
  if (reason instanceof ApiError) return `${reason.message} [${reason.code}]`
  if (reason instanceof Error) return reason.message
  return "An unexpected sync diagnostics error occurred."
}

export function SyncDiagnosticsPanel({ projectId, session }: SyncDiagnosticsPanelProps) {
  const [expanded, setExpanded] = useState(false)
  const [busy, setBusy] = useState(false)
  const [diagnostics, setDiagnostics] = useState<SyncDiagnostics | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE)
  const [mappingVisibleCount, setMappingVisibleCount] = useState(PAGE_SIZE)

  async function toggle(): Promise<void> {
    if (expanded) {
      setExpanded(false)
      return
    }
    setExpanded(true)
    if (diagnostics || busy) return
    setBusy(true)
    setError(null)
    try {
      setDiagnostics(await api.getSyncDiagnostics(projectId, session.id))
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setBusy(false)
    }
  }

  const segmentPage = diagnostics
    ? paginateDiagnostics(diagnosticSegmentRows(diagnostics), visibleCount)
    : null
  const mappingPage = paginateDiagnostics(
    unsupportedMappingRows(session.sources),
    mappingVisibleCount,
  )
  const triggerId = `sync-diagnostics-trigger-${session.id}`

  return (
    <section className="sync-diagnostics" aria-labelledby={triggerId}>
      <Button
        id={triggerId}
        type="button"
        variant="ghost"
        busy={busy}
        aria-label="Sync diagnostics"
        aria-expanded={expanded}
        onClick={() => void toggle()}
      >
        <Activity aria-hidden="true" size={13} /> Sync diagnostics
        <ChevronDown className={expanded ? "sync-diagnostics__chevron--open" : ""} size={13} />
      </Button>
      {expanded ? (
        <div className="sync-diagnostics__body">
          <header>
            <div>
              <p className="eyebrow">Declared metadata diagnostics</p>
              <h4>Sync diagnostics</h4>
            </div>
            {diagnostics ? <Badge>{diagnostics.calculation_status}</Badge> : null}
          </header>
          <p className="sync-diagnostics__disclaimer">
            Declared TimeMap relations only. This is not measured or verified alignment.
          </p>
          {error ? (
            <p className="sync-diagnostics__error" role="alert">
              {error}
            </p>
          ) : null}
          {busy ? <p className="muted">Loading declared diagnostics…</p> : null}
          {diagnostics ? (
            <>
              <dl className="sync-diagnostics__facts">
                <div>
                  <dt>TimeMaps</dt>
                  <dd>{diagnostics.adapter_capability.exact_time_maps ? "Declared" : "Unknown"}</dd>
                </div>
                <div>
                  <dt>Coverage</dt>
                  <dd>{diagnostics.coverage.status}</dd>
                </div>
                <div>
                  <dt>Items</dt>
                  <dd>
                    {diagnostics.limits.observed_items}/{diagnostics.limits.max_items}
                  </dd>
                </div>
              </dl>
              <SyncDiagnosticRanges diagnostics={diagnostics} />
              {mappingPage.visible.map((source) => (
                <p className="sync-diagnostics__unsupported" key={source.id}>
                  <strong>{source.displayName}:</strong> {source.reason}
                </p>
              ))}
              {mappingPage.remaining > 0 ? (
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => setMappingVisibleCount((count) => count + PAGE_SIZE)}
                >
                  Load 40 more unsupported mappings ({mappingPage.remaining} remaining)
                </Button>
              ) : null}
              <div className="sync-diagnostics__segments">
                {segmentPage?.visible.map((row) => (
                  <article key={row.id}>
                    <header>
                      <strong>{row.source}</strong>
                      <code>{row.stream}</code>
                    </header>
                    <span>
                      Session {row.sessionRange} {row.timeUnit}
                    </span>
                    <span>
                      Offset {row.offset} {row.timeUnit} · duration drift {row.durationDelta}{" "}
                      {row.timeUnit}
                    </span>
                    <span>
                      Rate ratio {row.relativeRate} · drift rate {row.relativeRatePpm}{" "}
                      {row.relativeRatePpmUnit}
                    </span>
                    <small>Declared confidence {row.confidence}</small>
                  </article>
                ))}
              </div>
              {segmentPage && segmentPage.remaining > 0 ? (
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => setVisibleCount((count) => count + PAGE_SIZE)}
                >
                  Load 40 more segments ({segmentPage.remaining} remaining)
                </Button>
              ) : null}
            </>
          ) : null}
        </div>
      ) : null}
    </section>
  )
}
