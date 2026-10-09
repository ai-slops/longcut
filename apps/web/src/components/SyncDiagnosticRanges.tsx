import { type ReactNode, useState } from "react"
import type {
  DiagnosticOverlapRow,
  DiagnosticRangeRow,
  SyncDiagnostics,
} from "../sync-diagnostics-domain"
import {
  diagnosticGapRows,
  diagnosticOverlapRows,
  exactValueLabel,
  paginateDiagnostics,
} from "../sync-diagnostics-domain"
import { Badge, Button } from "./Ui"

const PAGE_SIZE = 40

type PagedDiagnosticListProps<T extends { readonly id: string }> = {
  readonly title: string
  readonly rows: readonly T[]
  readonly renderRow: (row: T) => ReactNode
}

function PagedDiagnosticList<T extends { readonly id: string }>({
  title,
  rows,
  renderRow,
}: PagedDiagnosticListProps<T>) {
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE)
  const [expanded, setExpanded] = useState(rows.length <= PAGE_SIZE)
  const page = paginateDiagnostics(rows, visibleCount)
  return (
    <details
      className="sync-diagnostics__range-group"
      open={expanded}
      onToggle={(event) => setExpanded(event.currentTarget.open)}
    >
      <summary>
        <span>{title}</span>
        <Badge>{rows.length}</Badge>
      </summary>
      {rows.length === 0 ? (
        <p className="muted">No declared entries.</p>
      ) : (
        <ol>
          {page.visible.map((row) => (
            <li key={row.id}>{renderRow(row)}</li>
          ))}
        </ol>
      )}
      {page.remaining > 0 ? (
        <Button
          type="button"
          variant="ghost"
          onClick={() => setVisibleCount((count) => count + PAGE_SIZE)}
        >
          Load 40 more ({page.remaining} remaining)
        </Button>
      ) : null}
    </details>
  )
}

function gapLabel(row: DiagnosticRangeRow): ReactNode {
  return (
    <code>
      {row.range} {row.timeUnit}
    </code>
  )
}

function overlapLabel(row: DiagnosticOverlapRow): ReactNode {
  return (
    <span>
      <strong>
        {row.assetId} · stream {row.streamIndex}
      </strong>
      <code>
        {row.range} {row.timeUnit}
      </code>
    </span>
  )
}

export function SyncDiagnosticRanges({ diagnostics }: { readonly diagnostics: SyncDiagnostics }) {
  const overlaps = diagnosticOverlapRows(diagnostics)
  switch (diagnostics.coverage.status) {
    case "exact": {
      const gaps = diagnosticGapRows(diagnostics.coverage)
      return (
        <div className="sync-diagnostics__ranges">
          <div className="sync-diagnostics__coverage">
            <strong>Coverage union · not per-stream coverage</strong>
            <span>
              Declared session domain {exactValueLabel(diagnostics.coverage.domain.start)} –{" "}
              {exactValueLabel(diagnostics.coverage.domain.end)} seconds
            </span>
            <span>{diagnostics.coverage.ranges.length} merged union range(s)</span>
          </div>
          <PagedDiagnosticList title="Coverage-union gaps" rows={gaps} renderRow={gapLabel} />
          <PagedDiagnosticList
            title="Same-stream overlaps"
            rows={overlaps}
            renderRow={overlapLabel}
          />
        </div>
      )
    }
    case "unknown":
    case "unverifiable":
      return (
        <div className="sync-diagnostics__ranges">
          <p className="sync-diagnostics__unknown">
            Coverage {diagnostics.coverage.status} —{" "}
            {diagnostics.coverage.reason.replaceAll("_", " ")}
          </p>
          <PagedDiagnosticList
            title="Same-stream overlaps"
            rows={overlaps}
            renderRow={overlapLabel}
          />
        </div>
      )
    default: {
      const exhaustive: never = diagnostics.coverage
      return exhaustive
    }
  }
}
