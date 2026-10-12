import { useEffect, useState } from "react"
import type { Chapter, Command } from "../domain"
import { createStoryCommand, reconcileStoryDraft, validateStoryText } from "../story-domain"
import { Button, Field, TextArea } from "./Ui"

type ChapterEditorProps = {
  readonly chapter: Chapter
  readonly storyVersion: string
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function ChapterEditor({ chapter, storyVersion, busy, onCommand }: ChapterEditorProps) {
  const [draft, setDraft] = useState({
    openedVersion: storyVersion,
    baseline: { title: chapter.title, purpose: chapter.purpose },
    fields: { title: chapter.title, purpose: chapter.purpose },
  })
  const stale = draft.openedVersion !== storyVersion
  const valid =
    validateStoryText(draft.fields.title, "title").valid &&
    validateStoryText(draft.fields.purpose, "purpose").valid

  useEffect(() => {
    setDraft((current) =>
      reconcileStoryDraft(current, storyVersion, {
        title: chapter.title,
        purpose: chapter.purpose,
      }),
    )
  }, [chapter.purpose, chapter.title, storyVersion])

  function reload(): void {
    const fields = { title: chapter.title, purpose: chapter.purpose }
    setDraft({ openedVersion: storyVersion, baseline: fields, fields })
  }

  return (
    <form
      className="story-entity-form"
      onSubmit={(event) => {
        event.preventDefault()
        if (stale || !valid) return
        void onCommand(
          createStoryCommand(draft.openedVersion, {
            type: "update_chapter",
            chapter_id: chapter.id,
            title: draft.fields.title,
            purpose: draft.fields.purpose,
          }),
        )
      }}
    >
      <Field
        label="Chapter title"
        value={draft.fields.title}
        onChange={(event) => {
          const title = event.currentTarget.value
          setDraft((current) => ({ ...current, fields: { ...current.fields, title } }))
        }}
      />
      <TextArea
        label="Chapter purpose"
        rows={3}
        value={draft.fields.purpose}
        onChange={(event) => {
          const purpose = event.currentTarget.value
          setDraft((current) => ({ ...current, fields: { ...current.fields, purpose } }))
        }}
      />
      {stale ? (
        <p className="story-stale" role="status">
          The outline changed since this form opened. Your draft is preserved; reload before saving.
        </p>
      ) : null}
      <div className="story-form-actions">
        {stale ? (
          <Button type="button" variant="ghost" onClick={reload}>
            Reload current fields
          </Button>
        ) : null}
        <Button type="submit" disabled={busy || stale || !valid}>
          Save chapter
        </Button>
        <Button
          type="button"
          variant="danger"
          disabled={busy || stale || chapter.beats.length > 0}
          title={chapter.beats.length > 0 ? "Delete every beat in this chapter first." : undefined}
          onClick={() =>
            void onCommand(
              createStoryCommand(draft.openedVersion, {
                type: "delete_chapter",
                chapter_id: chapter.id,
              }),
            )
          }
        >
          Delete empty chapter
        </Button>
      </div>
      {chapter.beats.length > 0 ? (
        <small className="story-guard">
          Delete or move all {chapter.beats.length} beats before deleting this chapter.
        </small>
      ) : null}
    </form>
  )
}
