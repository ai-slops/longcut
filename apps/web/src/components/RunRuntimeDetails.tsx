import { ChevronDown } from "lucide-react"
import { useCallback, useEffect, useRef, useState } from "react"
import { aiApi } from "../ai-api"
import type { AiRun, RunRuntimeAvailability } from "../ai-domain"
import { ApiError } from "../api"
import { RuntimeRequestCoordinator } from "../runtime-request"

type RunRuntimeDetailsProps = {
  readonly projectId: string
  readonly run: AiRun
}

function runtimeError(reason: unknown): string {
  if (reason instanceof ApiError) return `${reason.message} [${reason.code}]`
  if (reason instanceof Error) return "Runtime details are temporarily unavailable."
  return "Runtime details are temporarily unavailable."
}

export function RunRuntimeDetails({ projectId, run }: RunRuntimeDetailsProps) {
  const [expanded, setExpanded] = useState(false)
  const [observation, setObservation] = useState<RunRuntimeAvailability | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const requests = useRef(new RuntimeRequestCoordinator())

  const refresh = useCallback(async (): Promise<void> => {
    const ticket = requests.current.begin({ projectId, runId: run.id })
    setLoading(true)
    try {
      const next = await aiApi.runRuntime(projectId, run.id, ticket.signal)
      if (!requests.current.owns(ticket)) return
      setObservation(next)
      setError(null)
    } catch (reason) {
      if (!requests.current.owns(ticket)) return
      setError(runtimeError(reason))
    } finally {
      if (requests.current.owns(ticket)) setLoading(false)
    }
  }, [projectId, run.id])

  useEffect(() => {
    if (!expanded) {
      requests.current.invalidate()
      return
    }
    void refresh()
    if (run.status !== "queued" && run.status !== "sampling" && run.status !== "generating") {
      return () => requests.current.invalidate()
    }
    const interval = window.setInterval(() => void refresh(), 2_000)
    return () => {
      window.clearInterval(interval)
      requests.current.invalidate()
    }
  }, [expanded, refresh, run.status])

  return (
    <section className="run-runtime">
      <button
        type="button"
        className="run-runtime__toggle"
        aria-expanded={expanded}
        onClick={() => setExpanded((current) => !current)}
      >
        Runtime details
        <ChevronDown aria-hidden="true" size={15} />
      </button>
      {expanded ? (
        <div className="run-runtime__body">
          {loading && observation === null ? (
            <p className="muted">Loading runtime details…</p>
          ) : null}
          {error ? <p className="runtime-status__error">{error}</p> : null}
          {observation?.available === false ? (
            <p>
              Runtime details are no longer available for this saved run. Its proposal remains
              unchanged.
            </p>
          ) : null}
          {observation?.available && observation.runtime && observation.progress ? (
            <dl className="agent-facts">
              <div>
                <dt>Engine</dt>
                <dd>
                  {observation.runtime.engine} {observation.runtime.engine_version}
                </dd>
              </div>
              <div>
                <dt>Execution</dt>
                <dd>{observation.runtime.execution_id}</dd>
              </div>
              <div>
                <dt>Profile revision</dt>
                <dd>{observation.runtime.profile_revision}</dd>
              </div>
              <div>
                <dt>Progress</dt>
                <dd>
                  {observation.progress.phase} · sequence {observation.progress.sequence}
                </dd>
              </div>
            </dl>
          ) : null}
        </div>
      ) : null}
    </section>
  )
}
