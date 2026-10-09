import { Check, Lock } from "lucide-react"
import type { FormEvent } from "react"
import type { Command, Project, Scene } from "../domain"
import { useDraft } from "../useDraft"
import { CameraEditor } from "./CameraEditor"
import { MomentVersionLink } from "./MomentVersionLink"
import { SceneComments } from "./SceneComments"
import { Badge, Button, Field, Panel, TextArea } from "./Ui"

type InspectorProps = {
  readonly project: Project
  readonly scene: Scene | null
  readonly busy: boolean
  readonly programFrame: number
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onSeek: (frame: number) => void
}

export function Inspector({
  project,
  scene,
  busy,
  programFrame,
  onCommand,
  onSeek,
}: InspectorProps) {
  const identity = scene?.id ?? ""
  const [title, setTitle] = useDraft(scene?.title ?? "", identity)
  const [purpose, setPurpose] = useDraft(scene?.purpose ?? "", identity)
  const [caption, setCaption] = useDraft(scene?.caption ?? "", identity)
  const [trimIn, setTrimIn] = useDraft(scene?.in_frame ?? "0", identity)
  const [trimOut, setTrimOut] = useDraft(scene?.out_frame ?? "0", identity)

  if (!scene) {
    return (
      <Panel title="Inspector" className="inspector">
        <div className="panel-empty">
          <Check size={22} />
          <strong>Nothing selected</strong>
          <p>Select a scene to edit metadata, camera geometry, and linked feedback.</p>
        </div>
      </Panel>
    )
  }

  async function updateScene(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (!scene) return
    await onCommand({ type: "update_scene", scene_id: scene.id, title, purpose, caption })
  }

  return (
    <aside className="inspector">
      <Panel
        title="Scene"
        tools={
          <div className="cluster">
            <Badge tone={scene.manual ? "violet" : "neutral"}>
              {scene.manual ? "Manual" : "Proposal-safe"}
            </Badge>
            {scene.locked ? <Lock size={14} /> : null}
          </div>
        }
      >
        <form className="stack" onSubmit={(event) => void updateScene(event)}>
          <Field
            label="Title"
            value={title}
            onChange={(event) => setTitle(event.currentTarget.value)}
            disabled={scene.locked}
          />
          <TextArea
            label="Purpose"
            value={purpose}
            onChange={(event) => setPurpose(event.currentTarget.value)}
            disabled={scene.locked}
            rows={2}
          />
          <TextArea
            label="Caption"
            value={caption}
            onChange={(event) => setCaption(event.currentTarget.value)}
            disabled={scene.locked}
            rows={2}
          />
          <Button type="submit" busy={busy} disabled={scene.locked}>
            Save scene
          </Button>
          {scene.moment ? (
            <div className="scene-moment-link">
              <Badge tone="violet">Moment v{scene.moment.version}</Badge>
              <p className="scene-moment-link__copy">
                Frozen provenance; later Moment edits never rewrite this Scene.
              </p>
              <MomentVersionLink
                projectId={project.id}
                momentId={scene.moment.moment_id}
                version={scene.moment.version}
              />
            </div>
          ) : null}
          <div className="geometry-grid">
            <Field
              label="Trim in frame"
              type="number"
              min="0"
              value={trimIn}
              onChange={(event) => setTrimIn(event.currentTarget.value)}
              disabled={scene.locked}
            />
            <Field
              label="Trim out frame"
              type="number"
              min="1"
              value={trimOut}
              onChange={(event) => setTrimOut(event.currentTarget.value)}
              disabled={scene.locked}
            />
          </div>
          <Button
            type="button"
            busy={busy}
            disabled={scene.locked || Number(trimOut) <= Number(trimIn)}
            onClick={() =>
              void onCommand({
                type: "trim_scene",
                scene_id: scene.id,
                in_frame: trimIn,
                out_frame: trimOut,
              })
            }
          >
            Apply trim
          </Button>
        </form>
      </Panel>
      <CameraEditor project={project} scene={scene} busy={busy} onCommand={onCommand} />
      <SceneComments
        project={project}
        scene={scene}
        programFrame={programFrame}
        busy={busy}
        onCommand={onCommand}
        onSeek={onSeek}
      />
    </aside>
  )
}
