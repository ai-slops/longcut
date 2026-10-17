import { Cpu } from "lucide-react"
import type { RuntimeAvailability } from "../ai-domain"
import { Badge, Panel } from "./Ui"

type RuntimeAvailabilityPanelProps = {
  readonly status: RuntimeAvailability | null
  readonly loading: boolean
}

export function RuntimeAvailabilityPanel({ status, loading }: RuntimeAvailabilityPanelProps) {
  const available = status?.available === true
  return (
    <Panel
      title="PART runtime"
      tools={
        <Badge tone={available ? "success" : loading ? "neutral" : "warning"}>
          {loading ? "checking" : available ? "available" : "unavailable"}
        </Badge>
      }
    >
      <div className="runtime-status">
        <header>
          <Cpu aria-hidden="true" size={16} />
          <strong>
            {loading
              ? "Checking execution service"
              : available
                ? "Execution service online"
                : "Execution service unavailable"}
          </strong>
        </header>
        <dl className="agent-facts">
          <div>
            <dt>Engine</dt>
            <dd>{status?.engine ?? "@slop-lab/part"}</dd>
          </div>
          <div>
            <dt>Version</dt>
            <dd>{status?.version ?? "Not reported"}</dd>
          </div>
        </dl>
        {status?.error ? <p className="runtime-status__error">{status.error}</p> : null}
        <p>Runtime availability does not confirm provider configuration or account access.</p>
      </div>
    </Panel>
  )
}
