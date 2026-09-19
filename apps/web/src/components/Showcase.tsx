import { Film } from "lucide-react"
import { Badge, Button, ErrorBar, Field, Panel, TextArea } from "./Ui"

export function Showcase() {
  return (
    <main className="showcase">
      <header>
        <p className="eyebrow">Design system harness</p>
        <h1>Longcut primitives</h1>
        <p>Production components and their required states.</p>
      </header>
      <section>
        <h2>Actions</h2>
        <div className="cluster">
          <Button variant="primary">Primary</Button>
          <Button>Secondary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="danger">Delete</Button>
          <Button busy>Working</Button>
          <Button disabled>Disabled</Button>
        </div>
      </section>
      <section>
        <h2>Status</h2>
        <div className="cluster">
          <Badge>Neutral</Badge>
          <Badge tone="lime">Current</Badge>
          <Badge tone="violet">Selected</Badge>
          <Badge tone="success">Ready</Badge>
          <Badge tone="warning">Processing</Badge>
          <Badge tone="error">Error</Badge>
        </div>
      </section>
      <section className="showcase__grid">
        <Panel title="Fields">
          <div className="stack">
            <Field label="Scene title" defaultValue="Opening context" />
            <TextArea label="Purpose" defaultValue="Establish the speaker and location." />
            <Field label="Disabled" disabled defaultValue="Locked scene" />
          </div>
        </Panel>
        <Panel title="Empty state">
          <div className="panel-empty">
            <Film size={22} />
            <strong>No source media</strong>
            <p>Import a local file to begin.</p>
          </div>
        </Panel>
      </section>
      <ErrorBar
        message="A stale revision must be refreshed before this edit can apply."
        onDismiss={() => undefined}
      />
    </main>
  )
}
