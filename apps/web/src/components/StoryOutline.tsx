import { BookOpen, Plus } from "lucide-react"
import { useEffect, useState } from "react"
import type { Chapter, Command, Project } from "../domain"
import { createStoryCommand, storyLimits, validateStoryText } from "../story-domain"
import { BeatDetail } from "./BeatDetail"
import { Badge, Button, Field, Panel } from "./Ui"

type StoryOutlineProps = {
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function StoryOutline({ project, busy, onCommand }: StoryOutlineProps) {
  const [chapterId, setChapterId] = useState<string | null>(project.story.chapters[0]?.id ?? null)
  const [beatId, setBeatId] = useState<string | null>(
    project.story.chapters[0]?.beats[0]?.id ?? null,
  )
  const [newTitle, setNewTitle] = useState("")
  const chapters = project.story.chapters
  const chapter = chapters.find((item) => item.id === chapterId) ?? null
  const beat = chapter?.beats.find((item) => item.id === beatId) ?? null
  const beatCount = chapters.reduce((count, item) => count + item.beats.length, 0)

  useEffect(() => {
    if (chapterId !== null && chapters.some((item) => item.id === chapterId)) return
    const next = chapters[0] ?? null
    setChapterId(next?.id ?? null)
    setBeatId(next?.beats[0]?.id ?? null)
  }, [chapterId, chapters])

  return (
    <Panel
      title="Manual outline"
      className="story-outline"
      tools={
        <Badge tone="violet">
          v{project.story.version} · {chapters.length}/{storyLimits.chapters} chapters
        </Badge>
      }
    >
      <p className="story-trust-copy">
        Outline order does not change playback order. Scenes remain authoritative.
      </p>
      <form
        className="story-create-row"
        onSubmit={(event) => {
          event.preventDefault()
          if (!validateStoryText(newTitle, "title").valid) return
          void onCommand(
            createStoryCommand(project.story.version, {
              type: "create_chapter",
              title: newTitle,
              purpose: "",
              before_chapter_id: null,
            }),
          ).then((saved) => {
            if (saved) setNewTitle("")
          })
        }}
      >
        <Field
          label="New chapter title"
          value={newTitle}
          maxLength={200}
          onChange={(event) => setNewTitle(event.currentTarget.value)}
        />
        <Button
          type="submit"
          disabled={
            busy ||
            chapters.length >= storyLimits.chapters ||
            !validateStoryText(newTitle, "title").valid
          }
        >
          <Plus size={14} /> Add chapter
        </Button>
      </form>
      <div className="story-outline__grid">
        <nav className="chapter-rail" aria-label="Story chapters">
          {chapters.length === 0 ? (
            <div className="panel-empty">
              <BookOpen size={22} />
              <strong>No chapters</strong>
              <p>Add a chapter to begin the manual outline.</p>
            </div>
          ) : (
            chapters.map((item, index) => (
              <ChapterRow
                key={item.id}
                chapter={item}
                index={index}
                count={chapters.length}
                selected={item.id === chapterId}
                busy={busy}
                storyVersion={project.story.version}
                onSelect={() => {
                  setChapterId(item.id)
                  setBeatId(item.beats[0]?.id ?? null)
                }}
                onCommand={onCommand}
                nextChapterId={chapters[index + 2]?.id ?? null}
                previousChapterId={chapters[index - 1]?.id ?? null}
              />
            ))
          )}
        </nav>
        {chapter ? (
          <BeatDetail
            key={chapter.id}
            project={project}
            chapter={chapter}
            beat={beat}
            selectedBeatId={beatId}
            beatCount={beatCount}
            busy={busy}
            onSelectBeat={setBeatId}
            onCommand={onCommand}
          />
        ) : (
          <div className="story-detail-empty">Select or create a chapter to edit its beats.</div>
        )}
      </div>
    </Panel>
  )
}

type ChapterRowProps = {
  readonly chapter: Chapter
  readonly index: number
  readonly count: number
  readonly selected: boolean
  readonly busy: boolean
  readonly storyVersion: string
  readonly previousChapterId: string | null
  readonly nextChapterId: string | null
  readonly onSelect: () => void
  readonly onCommand: (command: Command) => Promise<boolean>
}

function ChapterRow(props: ChapterRowProps) {
  const {
    chapter,
    index,
    count,
    selected,
    busy,
    storyVersion,
    previousChapterId,
    nextChapterId,
    onSelect,
    onCommand,
  } = props
  return (
    <article className={selected ? "selected" : ""}>
      <button
        type="button"
        className="chapter-rail__select"
        aria-current={selected ? "true" : undefined}
        onClick={onSelect}
      >
        <span>{String(index + 1).padStart(2, "0")}</span>
        <strong>{chapter.title}</strong>
        <small>{chapter.beats.length} beats</small>
      </button>
      <div className="row-actions">
        <button
          type="button"
          disabled={busy || index === 0}
          onClick={() =>
            void onCommand(
              createStoryCommand(storyVersion, {
                type: "move_chapter",
                chapter_id: chapter.id,
                before_chapter_id: previousChapterId,
              }),
            )
          }
        >
          Earlier
        </button>
        <button
          type="button"
          disabled={busy || index === count - 1}
          onClick={() =>
            void onCommand(
              createStoryCommand(storyVersion, {
                type: "move_chapter",
                chapter_id: chapter.id,
                before_chapter_id: nextChapterId,
              }),
            )
          }
        >
          Later
        </button>
      </div>
    </article>
  )
}
