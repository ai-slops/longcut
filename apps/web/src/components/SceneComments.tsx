import { MessageSquarePlus } from "lucide-react"
import { type FormEvent, type PointerEvent, useEffect, useRef, useState } from "react"
import type { Command, Project, Scene } from "../domain"
import { sceneDuration, sceneProgramStart } from "../timeline"
import { Badge, Button, Field, Panel, TextArea } from "./Ui"

type Props = {
  readonly project: Project
  readonly scene: Scene
  readonly programFrame: number
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onSeek: (frame: number) => void
}

export function SceneComments({ project, scene, programFrame, busy, onCommand, onSeek }: Props) {
  const duration = sceneDuration(scene)
  const sceneStart = sceneProgramStart(project, scene.id)
  const local = Math.max(0, Math.min(duration - 1, programFrame - sceneStart))
  const feedback = project.feedback.filter((item) => item.scene_id === scene.id)
  const readyRenders = project.renders.filter((render) => render.status === "ready")
  const [comment, setComment] = useState("")
  const [start, setStart] = useState(String(local))
  const [end, setEnd] = useState(String(Math.min(duration, local + 1)))
  const [pin, setPin] = useState<{ x: number; y: number } | null>(null)
  const [renderId, setRenderId] = useState("")
  const draftSceneId = useRef<string | null>(null)

  useEffect(() => {
    if (draftSceneId.current === scene.id) return
    draftSceneId.current = scene.id
    setStart(String(local))
    setEnd(String(Math.min(duration, local + 1)))
  }, [duration, local, scene.id])

  function placePin(event: PointerEvent<HTMLButtonElement>): void {
    const bounds = event.currentTarget.getBoundingClientRect()
    setPin({
      x: Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)),
      y: Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height)),
    })
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (!comment.trim()) return
    const succeeded = await onCommand({
      type: "create_feedback",
      scene_id: scene.id,
      start_frame: start,
      end_frame: end,
      body: comment.trim(),
      pin,
      render_id: renderId || null,
    })
    if (succeeded) setComment("")
  }

  return (
    <Panel title="Comments" tools={<Badge>{feedback.length}</Badge>}>
      <form className="stack" onSubmit={(event) => void submit(event)}>
        <TextArea
          label="Range comment"
          value={comment}
          onChange={(event) => setComment(event.currentTarget.value)}
          placeholder="What should change in this scene?"
          rows={2}
        />
        <div className="geometry-grid">
          <Field
            label="Start frame"
            type="number"
            min="0"
            max={duration - 1}
            value={start}
            onChange={(event) => setStart(event.currentTarget.value)}
          />
          <Field
            label="End frame"
            type="number"
            min="1"
            max={duration}
            value={end}
            onChange={(event) => setEnd(event.currentTarget.value)}
          />
        </div>
        <button
          type="button"
          className="pin-picker"
          onPointerDown={placePin}
          aria-label="Place normalized frame pin"
        >
          <span>Click to place frame pin</span>
          {pin ? (
            <i
              style={{ insetInlineStart: `${pin.x * 100}%`, insetBlockStart: `${pin.y * 100}%` }}
            />
          ) : null}
        </button>
        <label className="field">
          <span>Reference</span>
          <select value={renderId} onChange={(event) => setRenderId(event.currentTarget.value)}>
            <option value="">Current edit</option>
            {readyRenders.map((render) => (
              <option key={render.id} value={render.id}>
                {render.kind} r{render.revision}
              </option>
            ))}
          </select>
        </label>
        <Button type="submit" busy={busy} disabled={Number(end) <= Number(start)}>
          <MessageSquarePlus size={14} /> Add range comment
        </Button>
      </form>
      <div className="comment-list">
        {feedback.map((item) => (
          <article className={item.resolved ? "resolved" : ""} key={item.id}>
            <p>{item.body}</p>
            <button type="button" onClick={() => onSeek(sceneStart + Number(item.start_frame))}>
              Seek to range
            </button>
            <footer>
              <span>
                {item.render_id ? `render ${item.render_id.slice(0, 8)}` : `edit r${item.revision}`}
              </span>
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation()
                  void onCommand({
                    type: "resolve_feedback",
                    feedback_id: item.id,
                    resolved: !item.resolved,
                  })
                }}
              >
                {item.resolved ? "Reopen" : "Resolve"}
              </button>
            </footer>
          </article>
        ))}
      </div>
    </Panel>
  )
}
