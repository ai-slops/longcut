import { Film } from "lucide-react"
import type { Command, Project } from "../domain"
import { useDraft } from "../useDraft"
import { Badge, Button, formatFrames, Panel, TextArea } from "./Ui"

type StoryPanelProps = {
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function StoryPanel({ project, busy, onCommand }: StoryPanelProps) {
  const [brief, setBrief] = useDraft(project.brief, project.id)
  return (
    <div className="workspace-panel story-panel">
      <Panel title="Editorial brief" tools={<Badge>Project-wide</Badge>}>
        <TextArea
          label="What must this edit preserve?"
          value={brief}
          onChange={(event) => setBrief(event.currentTarget.value)}
          rows={4}
          placeholder="Keep the full final attempt."
        />
        <Button
          type="button"
          busy={busy}
          onClick={() => void onCommand({ type: "set_brief", brief })}
        >
          Save brief
        </Button>
      </Panel>
      <Panel
        title="Scene order"
        tools={<Badge tone="violet">{project.scenes.length} scenes</Badge>}
      >
        <div className="story-list">
          {project.scenes.length === 0 ? (
            <div className="panel-empty">
              <Film size={22} />
              <strong>No story yet</strong>
              <p>Build scenes from exact ranges in Edit.</p>
            </div>
          ) : (
            project.scenes.map((scene, index) => (
              <article key={scene.id}>
                <span>{String(index + 1).padStart(2, "0")}</span>
                <div>
                  <strong>{scene.title}</strong>
                  <p>{scene.purpose || "No purpose written"}</p>
                </div>
                <code>
                  {formatFrames(
                    String(Number(scene.out_frame) - Number(scene.in_frame)),
                    project.frame_rate,
                  )}
                </code>
                <div className="row-actions">
                  <button
                    type="button"
                    disabled={index === 0 || busy}
                    onClick={() => {
                      const before = project.scenes[index - 1]
                      if (before)
                        void onCommand({
                          type: "move_scene",
                          scene_id: scene.id,
                          before_scene_id: before.id,
                        })
                    }}
                  >
                    Earlier
                  </button>
                  <button
                    type="button"
                    disabled={index === project.scenes.length - 1 || busy}
                    onClick={() => {
                      const target = project.scenes[index + 2]
                      void onCommand({
                        type: "move_scene",
                        scene_id: scene.id,
                        before_scene_id: target?.id ?? null,
                      })
                    }}
                  >
                    Later
                  </button>
                </div>
              </article>
            ))
          )}
        </div>
      </Panel>
    </div>
  )
}
