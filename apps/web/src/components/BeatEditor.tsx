import { useEffect, useState } from "react"
import type { Beat, Chapter, Command, Project } from "../domain"
import {
  budgetSummary,
  createStoryCommand,
  reconcileStoryDraft,
  validateStoryText,
  validateTargetFrames,
} from "../story-domain"
import { Badge, Button, Field, TextArea } from "./Ui"

type BeatEditorProps = {
  readonly project: Project
  readonly chapter: Chapter
  readonly beat: Beat
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function BeatEditor({ project, chapter, beat, busy, onCommand }: BeatEditorProps) {
  const [draft, setDraft] = useState({
    openedVersion: project.story.version,
    baseline: {
      title: beat.title,
      purpose: beat.purpose,
      targetFrames: beat.target_frames ?? "",
    },
    fields: {
      title: beat.title,
      purpose: beat.purpose,
      targetFrames: beat.target_frames ?? "",
    },
  })
  const stale = draft.openedVersion !== project.story.version
  const target = validateTargetFrames(draft.fields.targetFrames)
  const assigned = project.scenes.filter((scene) => scene.beat_id === beat.id)
  const budget = budgetSummary(beat.target_frames, assigned)
  const valid =
    validateStoryText(draft.fields.title, "title").valid &&
    validateStoryText(draft.fields.purpose, "purpose").valid &&
    target !== false

  useEffect(() => {
    setDraft((current) =>
      reconcileStoryDraft(current, project.story.version, {
        title: beat.title,
        purpose: beat.purpose,
        targetFrames: beat.target_frames ?? "",
      }),
    )
  }, [beat.purpose, beat.target_frames, beat.title, project.story.version])

  function reload(): void {
    const fields = {
      title: beat.title,
      purpose: beat.purpose,
      targetFrames: beat.target_frames ?? "",
    }
    setDraft({
      openedVersion: project.story.version,
      baseline: fields,
      fields,
    })
  }

  return (
    <section className="beat-editor">
      <header>
        <div>
          <span>Selected beat</span>
          <h3>{beat.title}</h3>
        </div>
        <div className="cluster">
          <Badge tone="violet">{assigned.length} scenes</Badge>
          <Badge tone={budget.delta?.startsWith("+") ? "warning" : "neutral"}>
            {budget.target
              ? `${budget.actual} / ${budget.target} frames · Δ ${budget.delta}`
              : `${budget.actual} actual frames · no target`}
          </Badge>
        </div>
      </header>
      <form
        className="story-entity-form"
        onSubmit={(event) => {
          event.preventDefault()
          if (stale || !valid) return
          void onCommand(
            createStoryCommand(draft.openedVersion, {
              type: "update_beat",
              beat_id: beat.id,
              title: draft.fields.title,
              purpose: draft.fields.purpose,
              target_frames: target,
            }),
          )
        }}
      >
        <Field
          label="Beat title"
          value={draft.fields.title}
          onChange={(event) => {
            const title = event.currentTarget.value
            setDraft((current) => ({ ...current, fields: { ...current.fields, title } }))
          }}
        />
        <TextArea
          label="Beat purpose"
          rows={3}
          value={draft.fields.purpose}
          onChange={(event) => {
            const purpose = event.currentTarget.value
            setDraft((current) => ({ ...current, fields: { ...current.fields, purpose } }))
          }}
        />
        <Field
          label="Advisory target frames"
          inputMode="numeric"
          value={draft.fields.targetFrames}
          hint="Optional: 1–2147483647. This never trims scenes."
          onChange={(event) => {
            const targetFrames = event.currentTarget.value
            setDraft((current) => ({
              ...current,
              fields: { ...current.fields, targetFrames },
            }))
          }}
        />
        {target === false ? (
          <p className="error-copy" role="alert">
            Enter a positive whole-frame value up to 2147483647, or leave it empty.
          </p>
        ) : null}
        {stale ? (
          <p className="story-stale" role="status">
            The outline changed since this beat opened. Your draft is preserved; reload before
            saving.
          </p>
        ) : null}
        <label className="field">
          <span>Move to chapter</span>
          <select
            value={chapter.id}
            disabled={busy || stale}
            onChange={(event) =>
              void onCommand(
                createStoryCommand(draft.openedVersion, {
                  type: "move_beat",
                  beat_id: beat.id,
                  chapter_id: event.currentTarget.value,
                  before_beat_id: null,
                }),
              )
            }
          >
            {project.story.chapters.map((item) => (
              <option key={item.id} value={item.id}>
                {item.title}
              </option>
            ))}
          </select>
        </label>
        <div className="story-form-actions">
          {stale ? (
            <Button type="button" variant="ghost" onClick={reload}>
              Reload current fields
            </Button>
          ) : null}
          <Button type="submit" disabled={busy || stale || !valid}>
            Save beat
          </Button>
          <Button
            type="button"
            variant="danger"
            disabled={busy || stale || assigned.length > 0}
            title={assigned.length > 0 ? "Unassign every scene from this beat first." : undefined}
            onClick={() =>
              void onCommand(
                createStoryCommand(draft.openedVersion, { type: "delete_beat", beat_id: beat.id }),
              )
            }
          >
            Delete unassigned beat
          </Button>
        </div>
        {assigned.length > 0 ? (
          <small className="story-guard">
            Unassign {assigned.length} scenes before deleting this beat.
          </small>
        ) : null}
      </form>
    </section>
  )
}
