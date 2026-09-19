import { Captions, Download, FileJson2, Film } from "lucide-react"
import type { Project } from "../domain"
import { RenderViewer } from "./RenderViewer"
import { Badge, Button } from "./Ui"

type DeliverablesPanelProps = {
  readonly project: Project
  readonly busy: boolean
  readonly onRender: (kind: "preview" | "final") => Promise<void>
}

export function DeliverablesPanel({ project, busy, onRender }: DeliverablesPanelProps) {
  return (
    <div className="workspace-panel deliverables">
      <header className="deliverables__hero">
        <div>
          <p className="eyebrow">Revision-pinned output</p>
          <h2>Exact renders and evidence</h2>
          <p>Preview and final use the same compositor. Final exports are 1920×1080 H.264/AAC.</p>
        </div>
        <div>
          <Button type="button" busy={busy} onClick={() => void onRender("preview")}>
            Render preview
          </Button>
          <Button
            type="button"
            variant="primary"
            busy={busy}
            onClick={() => void onRender("final")}
          >
            Render final
          </Button>
        </div>
      </header>
      <RenderViewer renders={project.renders} />
      <div className="render-list">
        {project.renders.length === 0 ? (
          <div className="panel-empty">
            <Film size={22} />
            <strong>No exact render yet</strong>
            <p>Scene playback is approximate until a render is ready.</p>
          </div>
        ) : (
          project.renders.map((render) => (
            <article key={render.id}>
              <span className="render-icon">
                <Film size={20} />
              </span>
              <div>
                <header>
                  <strong>
                    {render.kind} · revision {render.revision}
                  </strong>
                  <Badge
                    tone={
                      render.status === "ready"
                        ? "success"
                        : render.status === "error"
                          ? "error"
                          : "warning"
                    }
                  >
                    {render.status}
                  </Badge>
                </header>
                {render.status === "running" || render.status === "queued" ? (
                  <div className="progress">
                    <i style={{ inlineSize: `${render.progress * 100}%` }} />
                  </div>
                ) : null}
                {render.error ? <p className="error-copy">{render.error}</p> : null}
                <footer>
                  {render.media_url ? (
                    <a href={render.media_url} download>
                      <Download size={13} /> Video
                    </a>
                  ) : null}
                  {render.srt_url ? (
                    <a href={render.srt_url} download>
                      <Captions size={13} /> SRT
                    </a>
                  ) : null}
                  {render.vtt_url ? (
                    <a href={render.vtt_url} download>
                      <Captions size={13} /> VTT
                    </a>
                  ) : null}
                  {render.lineage_url ? (
                    <a href={render.lineage_url} download>
                      <FileJson2 size={13} /> Lineage
                    </a>
                  ) : null}
                </footer>
              </div>
            </article>
          ))
        )}
      </div>
    </div>
  )
}
