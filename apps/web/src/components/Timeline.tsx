import { Lock, Scissors, Trash2, Unlock, ZoomIn, ZoomOut } from "lucide-react"
import { type MouseEvent, useState } from "react"
import type { Project, Scene } from "../domain"
import { Button, formatFrames } from "./Ui"

type TimelineProps = {
  readonly project: Project
  readonly selectedId: string | null
  readonly playhead: number
  readonly onPlayhead: (frame: number) => void
  readonly onSelect: (scene: Scene) => void
  readonly onDelete: (scene: Scene) => void
  readonly onSplit: (scene: Scene) => void
  readonly onLock: (scene: Scene) => void
}

export function Timeline({
  project,
  selectedId,
  playhead,
  onPlayhead,
  onSelect,
  onDelete,
  onSplit,
  onLock,
}: TimelineProps) {
  const [zoom, setZoom] = useState(1)
  const [skimmer, setSkimmer] = useState<number | null>(null)
  const durations = project.scenes.map((scene) => Number(scene.out_frame) - Number(scene.in_frame))
  const duration = durations.reduce((sum, length) => sum + length, 0)
  const total = Math.max(duration, 1)

  function position(event: MouseEvent<HTMLDivElement>): number {
    const rect = event.currentTarget.getBoundingClientRect()
    return Math.max(
      0,
      Math.min(
        Math.max(0, duration - 1),
        Math.round(((event.clientX - rect.left) / rect.width) * total),
      ),
    )
  }

  return (
    <section className="timeline" aria-label="Program timeline">
      <header className="timeline__toolbar">
        <div>
          <strong>Timeline</strong>
          <code>{formatFrames(String(playhead), project.frame_rate)}</code>
        </div>
        <div className="timeline__zoom">
          <button
            type="button"
            aria-label="Zoom out"
            onClick={() => setZoom(Math.max(0.75, zoom - 0.25))}
          >
            <ZoomOut size={14} />
          </button>
          <span>{Math.round(zoom * 100)}%</span>
          <button
            type="button"
            aria-label="Zoom in"
            onClick={() => setZoom(Math.min(2.5, zoom + 0.25))}
          >
            <ZoomIn size={14} />
          </button>
        </div>
      </header>
      <div className="timeline__scroller">
        <div
          className="timeline__canvas"
          style={{ inlineSize: `${zoom * 100}%` }}
          onPointerMove={(event) => setSkimmer(position(event))}
          onPointerLeave={() => setSkimmer(null)}
        >
          <input
            className="timeline-scrubber"
            type="range"
            min="0"
            max={Math.max(0, duration - 1)}
            disabled={duration === 0}
            value={playhead}
            aria-label="Program playhead"
            onChange={(event) => onPlayhead(Number(event.currentTarget.value))}
          />
          <div className="ruler">
            {[0, 0.25, 0.5, 0.75, 1].map((ratio) => (
              <span key={ratio} style={{ insetInlineStart: `${ratio * 100}%` }}>
                {formatFrames(String(Math.round(total * ratio)), project.frame_rate)}
              </span>
            ))}
          </div>
          <div className="track-label">V1</div>
          <div className="clip-track" role="listbox" aria-label="Timeline scenes">
            {project.scenes.length === 0 ? (
              <div className="track-empty">Mark In and Out on a source, then add a scene.</div>
            ) : (
              project.scenes.map((scene, index) => {
                const width = ((durations[index] ?? 0) / total) * 100
                return (
                  <button
                    type="button"
                    key={scene.id}
                    className={`clip ${selectedId === scene.id ? "selected" : ""}`}
                    role="option"
                    aria-selected={selectedId === scene.id}
                    aria-label={`${scene.title}, source frames ${scene.in_frame} to ${scene.out_frame}`}
                    title={`${scene.title}: ${scene.in_frame}–${scene.out_frame}`}
                    style={{ inlineSize: `${width}%` }}
                    onClick={(event) => {
                      event.stopPropagation()
                      onSelect(scene)
                    }}
                  >
                    <span>{index + 1}</span>
                    <strong>{scene.title}</strong>
                    {scene.locked ? <Lock size={11} /> : null}
                    {scene.manual ? <i>M</i> : null}
                  </button>
                )
              })
            )}
          </div>
          <span className="playhead" style={{ insetInlineStart: `${(playhead / total) * 100}%` }}>
            <i />
          </span>
          {skimmer === null ? null : (
            <span className="skimmer" style={{ insetInlineStart: `${(skimmer / total) * 100}%` }} />
          )}
        </div>
      </div>
      <div className="timeline__editbar">
        <span>{selectedId ? "Scene selected" : "Select a scene to edit"}</span>
        <Button
          type="button"
          variant="ghost"
          disabled={!selectedId}
          onClick={() => {
            const scene = project.scenes.find((item) => item.id === selectedId)
            if (scene) onSplit(scene)
          }}
        >
          <Scissors size={14} /> Split at playhead
        </Button>
        <Button
          type="button"
          variant="ghost"
          disabled={!selectedId}
          onClick={() => {
            const scene = project.scenes.find((item) => item.id === selectedId)
            if (scene) onLock(scene)
          }}
        >
          {project.scenes.find((item) => item.id === selectedId)?.locked ? (
            <Unlock size={14} />
          ) : (
            <Lock size={14} />
          )}{" "}
          Lock
        </Button>
        <Button
          type="button"
          variant="danger"
          disabled={!selectedId}
          onClick={() => {
            const scene = project.scenes.find((item) => item.id === selectedId)
            if (scene) onDelete(scene)
          }}
        >
          <Trash2 size={14} /> Delete
        </Button>
      </div>
    </section>
  )
}
