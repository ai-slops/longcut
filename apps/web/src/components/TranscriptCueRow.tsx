import { ChevronDown, ChevronRight } from "lucide-react"
import { useState } from "react"
import type { CueSelection, TranscriptCue } from "../transcript-domain"
import { cueSelection, wordSelection } from "../transcript-domain"
import { formatFrames } from "./Ui"

type TranscriptCueRowProps = {
  readonly cue: TranscriptCue
  readonly transcriptId: string
  readonly frameRate: number
  readonly onSelect: (selection: CueSelection) => void
}

export function TranscriptCueRow({
  cue,
  transcriptId,
  frameRate,
  onSelect,
}: TranscriptCueRowProps) {
  const [expanded, setExpanded] = useState(false)
  const words = cue.words ?? []
  const hasWords = words.length > 0

  return (
    <li>
      <div className="transcript-cue__header">
        <button
          className="transcript-cue__select"
          type="button"
          onClick={() => onSelect(cueSelection(cue, transcriptId))}
        >
          <code>{formatFrames(cue.start_frame, frameRate)}</code>
          <span>{cue.text}</span>
        </button>
        {hasWords ? (
          <button
            className="transcript-cue__expand"
            type="button"
            aria-expanded={expanded}
            aria-label="Word timing for cue"
            onClick={() => setExpanded((value) => !value)}
          >
            {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
            Words
          </button>
        ) : null}
      </div>
      {expanded ? (
        <div className="word-timing">
          <small>Model-estimated word timing · not verified alignment</small>
          <div className="word-timing__units">
            {words.map((word) => {
              const selection = wordSelection(word, transcriptId)
              return selection ? (
                <button
                  className="word-timing__word"
                  type="button"
                  key={word.id}
                  title={`${word.start_frame}–${word.end_frame} source frames`}
                  onClick={() => onSelect(selection)}
                >
                  {word.text}
                </button>
              ) : (
                <span
                  className="word-timing__punctuation"
                  key={word.id}
                  title={`Preserved at source frame ${word.start_frame}; no positive cut interval`}
                >
                  {word.text}
                </span>
              )
            })}
          </div>
        </div>
      ) : null}
    </li>
  )
}
