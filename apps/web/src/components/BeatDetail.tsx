import { ListPlus } from "lucide-react"
import { useState } from "react"
import type { Beat, Chapter, Command, Project } from "../domain"
import { createStoryCommand, storyLimits, validateStoryText } from "../story-domain"
import { BeatEditor } from "./BeatEditor"
import { BeatMomentRefs } from "./BeatMomentRefs"
import { ChapterEditor } from "./ChapterEditor"
import { Badge, Button, Field } from "./Ui"

type BeatDetailProps = {
  readonly project: Project
  readonly chapter: Chapter
  readonly beat: Beat | null
  readonly selectedBeatId: string | null
  readonly beatCount: number
  readonly busy: boolean
  readonly onSelectBeat: (id: string) => void
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function BeatDetail(props: BeatDetailProps) {
  const { project, chapter, beat, selectedBeatId, beatCount, busy, onSelectBeat, onCommand } = props
  const [newTitle, setNewTitle] = useState("")
  const canAdd = chapter.beats.length < storyLimits.beatsPerChapter && beatCount < storyLimits.beats

  return (
    <section className="beat-detail">
      <header className="beat-detail__header">
        <div>
          <span>Selected chapter</span>
          <h3>{chapter.title}</h3>
        </div>
        <Badge>
          {chapter.beats.length}/{storyLimits.beatsPerChapter} beats
        </Badge>
      </header>
      <ChapterEditor
        chapter={chapter}
        storyVersion={project.story.version}
        busy={busy}
        onCommand={onCommand}
      />
      <form
        className="story-create-row"
        onSubmit={(event) => {
          event.preventDefault()
          if (!canAdd || !validateStoryText(newTitle, "title").valid) return
          void onCommand(
            createStoryCommand(project.story.version, {
              type: "create_beat",
              chapter_id: chapter.id,
              title: newTitle,
              purpose: "",
              target_frames: null,
              before_beat_id: null,
            }),
          ).then((saved) => {
            if (saved) setNewTitle("")
          })
        }}
      >
        <Field
          label="New beat title"
          value={newTitle}
          onChange={(event) => setNewTitle(event.currentTarget.value)}
        />
        <Button
          type="submit"
          disabled={busy || !canAdd || !validateStoryText(newTitle, "title").valid}
        >
          <ListPlus size={14} /> Add beat
        </Button>
      </form>
      {!canAdd ? (
        <p className="story-guard">Beat limit reached for this chapter or Story.</p>
      ) : null}
      <ol className="beat-list" aria-label={`Beats in ${chapter.title}`}>
        {chapter.beats.map((item, index) => (
          <li key={item.id} className={item.id === selectedBeatId ? "selected" : ""}>
            <button
              type="button"
              className="beat-list__select"
              aria-current={item.id === selectedBeatId ? "true" : undefined}
              onClick={() => onSelectBeat(item.id)}
            >
              <span>{String(index + 1).padStart(2, "0")}</span>
              <strong>{item.title}</strong>
              <small>
                {item.target_frames ? `${item.target_frames} target frames` : "No target"}
              </small>
            </button>
            <div className="row-actions">
              <button
                type="button"
                disabled={busy || index === 0}
                onClick={() =>
                  void onCommand(
                    createStoryCommand(project.story.version, {
                      type: "move_beat",
                      beat_id: item.id,
                      chapter_id: chapter.id,
                      before_beat_id: chapter.beats[index - 1]?.id ?? null,
                    }),
                  )
                }
              >
                Earlier
              </button>
              <button
                type="button"
                disabled={busy || index === chapter.beats.length - 1}
                onClick={() =>
                  void onCommand(
                    createStoryCommand(project.story.version, {
                      type: "move_beat",
                      beat_id: item.id,
                      chapter_id: chapter.id,
                      before_beat_id: chapter.beats[index + 2]?.id ?? null,
                    }),
                  )
                }
              >
                Later
              </button>
            </div>
          </li>
        ))}
      </ol>
      {beat ? (
        <div className="beat-inspector">
          <BeatEditor
            key={beat.id}
            project={project}
            chapter={chapter}
            beat={beat}
            busy={busy}
            onCommand={onCommand}
          />
          <BeatMomentRefs project={project} beat={beat} busy={busy} onCommand={onCommand} />
        </div>
      ) : (
        <p className="story-detail-empty">Add or select a beat to edit its details.</p>
      )}
    </section>
  )
}
