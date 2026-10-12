import { Film } from "lucide-react"
import { useState } from "react"
import type { Command, Project } from "../domain"
import { assignmentAvailability, createStoryCommand } from "../story-domain"
import { Badge, Button, Panel } from "./Ui"

const pageSize = 40

type SceneAssignmentPanelProps = {
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function SceneAssignmentPanel({ project, busy, onCommand }: SceneAssignmentPanelProps) {
  const [limit, setLimit] = useState(pageSize)
  const visible = project.scenes.slice(0, limit)
  const beats = project.story.chapters.flatMap((chapter) =>
    chapter.beats.map((beat) => ({ beat, chapter })),
  )

  return (
    <Panel
      title="Scene playback order"
      className="scene-assignment"
      tools={<Badge tone="violet">{project.scenes.length} scenes</Badge>}
    >
      <p className="story-trust-copy">
        Outline order does not change playback order. This list remains the output-authoritative
        scene array.
      </p>
      {visible.length === 0 ? (
        <div className="panel-empty">
          <Film size={22} />
          <strong>No scenes</strong>
          <p>Unassigned scenes are valid. Build scenes from Source ranges in Edit.</p>
        </div>
      ) : (
        <div className="scene-assignment__list">
          {visible.map((scene, index) => {
            const linked = beats.find(({ beat }) => beat.id === scene.beat_id)
            const availability = assignmentAvailability(scene, scene.beat_id)
            const actualFrames = (BigInt(scene.out_frame) - BigInt(scene.in_frame)).toString()
            return (
              <article key={scene.id}>
                <span>{String(index + 1).padStart(2, "0")}</span>
                <div className="scene-assignment__meta">
                  <strong>{scene.title}</strong>
                  <small>{actualFrames} actual frames</small>
                  <div className="cluster">
                    {linked ? (
                      <>
                        <Badge>{linked.chapter.title}</Badge>
                        <Badge tone="violet">{linked.beat.title}</Badge>
                      </>
                    ) : (
                      <Badge tone="warning">Unassigned</Badge>
                    )}
                    {scene.locked ? <Badge tone="warning">Locked</Badge> : null}
                  </div>
                </div>
                <label className="field">
                  <span>Assigned beat</span>
                  <select
                    value={scene.beat_id ?? ""}
                    disabled={busy || scene.locked}
                    aria-describedby={scene.locked ? `scene-lock-${scene.id}` : undefined}
                    onChange={(event) => {
                      const next = event.currentTarget.value || null
                      const nextAvailability = assignmentAvailability(scene, next)
                      if (!nextAvailability.enabled) return
                      void onCommand(
                        createStoryCommand(project.story.version, {
                          type: "set_scene_beat",
                          scene_id: scene.id,
                          beat_id: next,
                        }),
                      )
                    }}
                  >
                    <option value="">Unassigned</option>
                    {project.story.chapters.map((chapter) => (
                      <optgroup key={chapter.id} label={chapter.title}>
                        {chapter.beats.map((beat) => (
                          <option key={beat.id} value={beat.id}>
                            {beat.title}
                          </option>
                        ))}
                      </optgroup>
                    ))}
                  </select>
                  {scene.locked ? (
                    <small id={`scene-lock-${scene.id}`}>
                      Unlock this scene in Edit before changing its beat.
                    </small>
                  ) : availability.reason ? (
                    <small>{availability.reason}</small>
                  ) : null}
                </label>
                <div className="row-actions">
                  <button
                    type="button"
                    disabled={busy || scene.locked || index === 0}
                    title={scene.locked ? "Unlock this scene in Edit before moving it." : undefined}
                    onClick={() =>
                      void onCommand({
                        type: "move_scene",
                        scene_id: scene.id,
                        before_scene_id: project.scenes[index - 1]?.id ?? null,
                      })
                    }
                  >
                    Earlier
                  </button>
                  <button
                    type="button"
                    disabled={busy || scene.locked || index === project.scenes.length - 1}
                    title={scene.locked ? "Unlock this scene in Edit before moving it." : undefined}
                    onClick={() =>
                      void onCommand({
                        type: "move_scene",
                        scene_id: scene.id,
                        before_scene_id: project.scenes[index + 2]?.id ?? null,
                      })
                    }
                  >
                    Later
                  </button>
                </div>
              </article>
            )
          })}
        </div>
      )}
      {visible.length < project.scenes.length ? (
        <Button type="button" variant="ghost" onClick={() => setLimit((value) => value + pageSize)}>
          Show 40 more scenes
        </Button>
      ) : null}
    </Panel>
  )
}
