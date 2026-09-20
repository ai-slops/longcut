import { RotateCcw, Square, StepForward } from "lucide-react"
import type { AiRun } from "../ai-domain"
import { isActiveRun } from "../ai-helpers"
import { Badge, Button, Panel } from "./Ui"

const providerNames: Record<AiRun["provider"], string> = {
  codex: "Codex ChatGPT",
  "openai-compatible": "OpenAI-compatible API",
}

type TrackAiRunsProps = {
  readonly runs: readonly AiRun[]
  readonly loading: boolean
  readonly cancellingId: string | null
  readonly onCancel: (runId: string) => Promise<void>
  readonly onReview: () => void
  readonly onReset: () => void
}

function tone(status: AiRun["status"]): "neutral" | "success" | "warning" | "error" {
  if (status === "ready") return "success"
  if (status === "error" || status === "stale") return "error"
  if (status === "queued" || status === "sampling" || status === "generating") return "warning"
  return "neutral"
}

export function TrackAiRuns({
  runs,
  loading,
  cancellingId,
  onCancel,
  onReview,
  onReset,
}: TrackAiRunsProps) {
  return (
    <Panel title="AI runs" tools={<Badge>{runs.length}</Badge>}>
      {loading && runs.length === 0 ? <p className="muted">Loading run history…</p> : null}
      {!loading && runs.length === 0 ? (
        <div className="panel-empty">
          <StepForward size={22} />
          <strong>No AI runs</strong>
          <p>Your requests and their honest operational state will appear here.</p>
        </div>
      ) : null}
      <div className="ai-run-list">
        {runs.map((run) => (
          <article key={run.id}>
            <header>
              <div>
                <strong>{run.instruction}</strong>
                <small>
                  Revision {run.base_revision} ·{" "}
                  {run.scope.type === "source"
                    ? `source ${run.scope.source_id}`
                    : `${run.scope.scene_ids.length} scenes`}
                </small>
              </div>
              <Badge tone={tone(run.status)}>{run.status}</Badge>
            </header>
            {run.summary ? <p>{run.summary}</p> : null}
            <dl className="agent-facts">
              <div>
                <dt>Provider</dt>
                <dd>{providerNames[run.provider]}</dd>
              </div>
              <div>
                <dt>Updated</dt>
                <dd>{new Date(run.updated_at).toLocaleString()}</dd>
              </div>
              <div>
                <dt>Visual samples</dt>
                <dd>{run.sample_count}</dd>
              </div>
            </dl>
            {run.warnings.length ? (
              <ul className="run-warnings">
                {run.warnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            ) : null}
            {run.error ? <p className="error-copy">{run.error}</p> : null}
            <footer>
              {isActiveRun(run) ? (
                <Button
                  type="button"
                  variant="danger"
                  busy={cancellingId === run.id}
                  onClick={() => void onCancel(run.id)}
                >
                  <Square size={13} /> Cancel
                </Button>
              ) : null}
              {run.status === "ready" && run.change_set_id ? (
                <Button type="button" variant="primary" onClick={onReview}>
                  Review proposal
                </Button>
              ) : null}
              {run.status === "error" || run.status === "cancelled" || run.status === "stale" ? (
                <Button type="button" variant="ghost" onClick={onReset}>
                  <RotateCcw size={13} /> Start over
                </Button>
              ) : null}
            </footer>
          </article>
        ))}
      </div>
    </Panel>
  )
}
