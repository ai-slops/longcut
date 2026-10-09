import { Archive, Check, MapPin, Pencil, RotateCcw, X } from "lucide-react"
import { useState } from "react"
import type { Command, Moment, Project } from "../domain"
import {
  canInsertMoment,
  momentArchiveCommand,
  momentQuality,
  momentReviewCommand,
  momentSceneCommand,
} from "../moment-workflow"
import { MomentEditForm } from "./MomentEditForm"
import { MomentVersionLink } from "./MomentVersionLink"
import { Badge, Button, formatFrames } from "./Ui"

type MomentCardProps = {
  readonly project: Project
  readonly moment: Moment
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onReveal: (moment: Moment) => void
}

export function MomentCard({ project, moment, busy, onCommand, onReveal }: MomentCardProps) {
  const [editing, setEditing] = useState(false)
  const source = project.sources.find((item) => item.id === moment.source_id)
  const quality = momentQuality(moment)
  const insertable = canInsertMoment(moment, source)
  const insertReason = moment.archived
    ? "Restore before adding a Scene."
    : moment.review_status !== "reviewed"
      ? "Editorial review is required before adding a Scene."
      : !source || source.width === 0 || source.height === 0
        ? "Audio-only Moments cannot become visual Scenes."
        : null

  return (
    <article className={`moment-card ${moment.archived ? "moment-card--archived" : ""}`}>
      <header>
        <div>
          <div className="cluster">
            <Badge
              tone={
                moment.review_status === "reviewed"
                  ? "success"
                  : moment.review_status === "rejected"
                    ? "error"
                    : "warning"
              }
            >
              {moment.review_status}
            </Badge>
            {moment.archived ? <Badge>Archived</Badge> : null}
            <Badge tone="violet">{moment.narrative_role}</Badge>
          </div>
          <h3>{moment.label}</h3>
          <p>
            {source?.name ?? moment.source_id} ·{" "}
            {formatFrames(moment.start_frame, project.frame_rate)}–
            {formatFrames(moment.end_frame, project.frame_rate)}
          </p>
        </div>
        <Badge>v{moment.version}</Badge>
      </header>
      {moment.note ? <p className="moment-card__note">{moment.note}</p> : null}
      <div className="moment-card__quality">
        <strong>Editorial review only</strong>
        <span>Does not verify ASR, speech accuracy, synchronization, or audiovisual truth.</span>
        {quality.transcriptOrigin ? (
          <span>Transcript origin: {quality.transcriptOrigin} · unverified</span>
        ) : null}
      </div>
      {moment.evidence.map((item) => (
        <figure key={`${item.transcript_id}:${item.start_frame}`}>
          <blockquote>{item.quote}</blockquote>
          <figcaption>
            {item.origin} · frozen {item.timing_basis} · transcript {item.transcript_id}
          </figcaption>
        </figure>
      ))}
      <div className="moment-card__actions">
        <Button type="button" variant="ghost" onClick={() => onReveal(moment)}>
          <MapPin size={14} /> Reveal source
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => setEditing((value) => !value)}
          disabled={busy || moment.archived}
        >
          <Pencil size={14} /> Edit
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => void onCommand(momentReviewCommand(moment, "reviewed"))}
          disabled={busy || moment.archived}
        >
          <Check size={14} /> Review
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => void onCommand(momentReviewCommand(moment, "rejected"))}
          disabled={busy || moment.archived}
        >
          <X size={14} /> Reject
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => void onCommand(momentArchiveCommand(moment))}
          disabled={busy}
        >
          {moment.archived ? <RotateCcw size={14} /> : <Archive size={14} />}{" "}
          {moment.archived ? "Restore" : "Archive"}
        </Button>
        <Button
          type="button"
          variant="primary"
          onClick={() => void onCommand(momentSceneCommand(moment))}
          disabled={busy || !insertable}
        >
          Add as scene
        </Button>
      </div>
      {insertReason ? <small className="moment-card__gate">{insertReason}</small> : null}
      {editing ? (
        <MomentEditForm
          moment={moment}
          project={project}
          busy={busy}
          onCommand={onCommand}
          onClose={() => setEditing(false)}
        />
      ) : null}
      <MomentVersionLink projectId={project.id} momentId={moment.id} version={moment.version} />
    </article>
  )
}
