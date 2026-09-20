import { Check, X } from "lucide-react"
import { type FormEvent, useState } from "react"
import { describeOperation } from "../ai-helpers"
import type { Command, Operation, Project } from "../domain"
import { useDraft } from "../useDraft"
import { Badge, Button, Field, Panel, TextArea } from "./Ui"

type Props = {
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function ProposalPanel({ project, busy, onCommand }: Props) {
  const [sceneId, setSceneId] = useState(project.scenes[0]?.id ?? "")
  const scene = project.scenes.find((item) => item.id === sceneId)
  const [title, setTitle] = useState("Review edit")
  const [intent, setIntent] = useState("Propose explicit timing and camera changes.")
  const [rationale, setRationale] = useState("Editor-authored alternative for review.")
  const [trimIn, setTrimIn] = useDraft(scene?.in_frame ?? "0", sceneId)
  const [trimOut, setTrimOut] = useDraft(scene?.out_frame ?? "0", sceneId)
  const [includeCamera, setIncludeCamera] = useState(false)
  const cameraSources = project.sources.filter((source) => source.role === "camera")
  const [cameraSourceId, setCameraSourceId] = useState(cameraSources[0]?.id ?? "")
  const [selected, setSelected] = useState<Record<string, boolean>>({})

  async function create(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (!scene) return
    const operations: Operation[] = []
    if (trimIn !== scene.in_frame || trimOut !== scene.out_frame) {
      operations.push({
        id: crypto.randomUUID(),
        scene_id: scene.id,
        type: "trim_scene",
        in_frame: trimIn,
        out_frame: trimOut,
        rationale,
      })
    }
    if (includeCamera && cameraSourceId) {
      operations.push({
        id: crypto.randomUUID(),
        scene_id: scene.id,
        type: "set_camera",
        rationale,
        camera:
          scene.camera?.source_id === cameraSourceId
            ? scene.camera
            : {
                source_id: cameraSourceId,
                in_frame: "0",
                x: 0.68,
                y: 0.06,
                width: 0.26,
                height: 0.26,
                opacity: 1,
              },
      })
    }
    if (operations.length === 0) return
    await onCommand({
      type: "create_change_set",
      title,
      intent,
      operations,
      evidence: [`Editor-authored by current operator for scene ${scene.id}`],
    })
  }

  return (
    <Panel title="Typed proposals" tools={<Badge tone="lime">Explicit editor author</Badge>}>
      <form className="stack proposal-form" onSubmit={(event) => void create(event)}>
        <label className="field">
          <span>Target scene</span>
          <select value={sceneId} onChange={(event) => setSceneId(event.currentTarget.value)}>
            {project.scenes.map((item) => (
              <option key={item.id} value={item.id}>
                {item.title}
              </option>
            ))}
          </select>
        </label>
        <Field
          label="Proposal title"
          value={title}
          onChange={(event) => setTitle(event.currentTarget.value)}
        />
        <TextArea
          label="Intent"
          value={intent}
          onChange={(event) => setIntent(event.currentTarget.value)}
          rows={2}
        />
        <div className="geometry-grid">
          <Field
            label="Proposed in"
            type="number"
            value={trimIn}
            onChange={(event) => setTrimIn(event.currentTarget.value)}
          />
          <Field
            label="Proposed out"
            type="number"
            value={trimOut}
            onChange={(event) => setTrimOut(event.currentTarget.value)}
          />
        </div>
        <label className="check-field">
          <input
            type="checkbox"
            checked={includeCamera}
            onChange={(event) => setIncludeCamera(event.currentTarget.checked)}
          />
          <span>Include camera operation</span>
        </label>
        {includeCamera ? (
          <label className="field">
            <span>Camera source</span>
            <select
              value={cameraSourceId}
              onChange={(event) => setCameraSourceId(event.currentTarget.value)}
            >
              {cameraSources.map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <TextArea
          label="Rationale"
          value={rationale}
          onChange={(event) => setRationale(event.currentTarget.value)}
          rows={2}
        />
        <Button
          type="submit"
          disabled={
            !scene || (trimIn === scene.in_frame && trimOut === scene.out_frame && !includeCamera)
          }
          busy={busy}
        >
          Create proposal
        </Button>
      </form>
      <div className="proposal-list">
        {project.change_sets.map((set) => {
          const available = set.operations.filter(
            (operation) => !set.applied_operation_ids.includes(operation.id),
          )
          return (
            <article key={set.id}>
              <header>
                <div>
                  <strong>{set.title}</strong>
                  <p>{set.intent}</p>
                </div>
                <Badge
                  tone={
                    set.status === "rejected"
                      ? "error"
                      : set.status === "ready"
                        ? "warning"
                        : "success"
                  }
                >
                  {set.status}
                </Badge>
              </header>
              <ul>
                {set.operations.map((operation) => (
                  <li key={operation.id}>
                    <label className="check-field">
                      <input
                        type="checkbox"
                        disabled={set.applied_operation_ids.includes(operation.id)}
                        checked={
                          set.applied_operation_ids.includes(operation.id) ||
                          Boolean(selected[operation.id])
                        }
                        onChange={(event) =>
                          setSelected((value) => ({
                            ...value,
                            [operation.id]: event.currentTarget.checked,
                          }))
                        }
                      />
                      <span className="operation-copy">
                        <strong>{describeOperation(operation, project.frame_rate)}</strong>
                        <small>{operation.rationale}</small>
                      </span>
                    </label>
                  </li>
                ))}
              </ul>
              {available.length > 0 && set.status !== "rejected" ? (
                <footer>
                  <Button
                    type="button"
                    busy={busy}
                    disabled={!available.some((operation) => selected[operation.id])}
                    onClick={() =>
                      void onCommand({
                        type: "apply_change_set",
                        change_set_id: set.id,
                        operation_ids: available
                          .filter((operation) => selected[operation.id])
                          .map((operation) => operation.id),
                      })
                    }
                  >
                    <Check size={14} /> Apply selected
                  </Button>
                  <Button
                    type="button"
                    variant="danger"
                    busy={busy}
                    onClick={() =>
                      void onCommand({ type: "reject_change_set", change_set_id: set.id })
                    }
                  >
                    <X size={14} /> Reject
                  </Button>
                </footer>
              ) : null}
            </article>
          )
        })}
      </div>
    </Panel>
  )
}
