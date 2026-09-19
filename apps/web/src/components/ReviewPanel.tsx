import { MessageSquare } from "lucide-react"
import type { Command, Project } from "../domain"
import { sceneProgramStart } from "../timeline"
import { ProposalPanel } from "./ProposalPanel"
import { Badge, Button, formatFrames, Panel } from "./Ui"

type ReviewPanelProps = {
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onSeek?: (frame: number, sceneId: string) => void
}

export function ReviewPanel({ project, busy, onCommand, onSeek }: ReviewPanelProps) {
  return (
    <div className="workspace-panel review-grid">
      <Panel title="Review notes" tools={<Badge>{project.feedback.length}</Badge>}>
        {project.feedback.length === 0 ? (
          <div className="panel-empty">
            <MessageSquare size={22} />
            <strong>No feedback yet</strong>
            <p>Select a scene in Edit to attach an exact range comment.</p>
          </div>
        ) : (
          <div className="review-list">
            {project.feedback.map((item) => {
              const scene = project.scenes.find((entry) => entry.id === item.scene_id)
              const original = project.renders.find((render) => render.id === item.render_id)
              return (
                <article key={item.id} className={item.resolved ? "resolved" : ""}>
                  <header>
                    <strong>{scene?.title ?? "Preserved evidence"}</strong>
                    <Badge tone={item.resolved ? "success" : "warning"}>
                      {item.resolved ? "Resolved" : "Open"}
                    </Badge>
                  </header>
                  <p>{item.body}</p>
                  {original?.media_url ? (
                    <a href={original.media_url} target="_blank" rel="noreferrer">
                      Open original render r{original.revision}
                    </a>
                  ) : (
                    <Button
                      type="button"
                      variant="ghost"
                      disabled={!scene || !onSeek}
                      onClick={() =>
                        onSeek?.(
                          sceneProgramStart(project, item.scene_id) + Number(item.start_frame),
                          item.scene_id,
                        )
                      }
                    >
                      {scene ? "Seek to range" : "Scene removed; evidence preserved"}
                    </Button>
                  )}
                  <footer>
                    <code>
                      {formatFrames(item.start_frame, project.frame_rate)}–
                      {formatFrames(item.end_frame, project.frame_rate)}
                    </code>
                    <span>Revision {item.revision}</span>
                    <Button
                      type="button"
                      variant="ghost"
                      busy={busy}
                      onClick={() =>
                        void onCommand({
                          type: "resolve_feedback",
                          feedback_id: item.id,
                          resolved: !item.resolved,
                        })
                      }
                    >
                      {item.resolved ? "Reopen" : "Resolve"}
                    </Button>
                  </footer>
                </article>
              )
            })}
          </div>
        )}
      </Panel>
      <ProposalPanel project={project} busy={busy} onCommand={onCommand} />
    </div>
  )
}
