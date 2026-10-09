import { type FormEvent, useId, useState } from "react"
import { type Command, type Moment, NarrativeRoleSchema, type Project } from "../domain"
import { prepareMomentUpdate, validateMomentRange, validateMomentText } from "../moment-workflow"
import { MomentRangeStatus } from "./MomentRangeStatus"
import { Button, Field, TextArea } from "./Ui"

type MomentEditFormProps = {
  readonly moment: Moment
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onClose: () => void
}

const roles = [
  "context",
  "setup",
  "goal",
  "attempt",
  "failure",
  "insight",
  "payoff",
  "other",
] as const

export function MomentEditForm({ moment, project, busy, onCommand, onClose }: MomentEditFormProps) {
  const rangeMessageId = useId()
  const sourceFieldId = useId()
  const roleFieldId = useId()
  const [base] = useState(moment)
  const [sourceId, setSourceId] = useState(base.source_id)
  const [startFrame, setStartFrame] = useState(base.start_frame)
  const [endFrame, setEndFrame] = useState(base.end_frame)
  const [label, setLabel] = useState(base.label)
  const [note, setNote] = useState(base.note)
  const [role, setRole] = useState(base.narrative_role)
  const text = validateMomentText(label, note)
  const source = project.sources.find((item) => item.id === sourceId)
  const range = validateMomentRange({
    startFrame,
    endFrame,
    durationFrames: source?.duration_frames ?? "0",
  })
  const prepared = prepareMomentUpdate({
    base,
    currentVersion: moment.version,
    draft: { sourceId, startFrame, endFrame, label, note, narrativeRole: role },
  })
  const stale = prepared.kind === "stale"
  const commandReady = prepared.kind === "ready"
  const editingDisabled = busy || stale || moment.archived
  const clearsEvidence =
    base.evidence.length > 0 && prepared.kind === "ready" && prepared.command.evidence === null

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (!text.valid || !range.valid || !commandReady || moment.archived) return
    if (await onCommand(prepared.command)) onClose()
  }

  return (
    <form className="moment-edit" onSubmit={(event) => void submit(event)}>
      <div className="moment-edit__grid">
        <label className="field" htmlFor={sourceFieldId}>
          <span>Source</span>
          <select
            id={sourceFieldId}
            value={sourceId}
            disabled={editingDisabled}
            onChange={(event) => setSourceId(event.currentTarget.value)}
          >
            {project.sources.map((source) => (
              <option key={source.id} value={source.id}>
                {source.name}
              </option>
            ))}
          </select>
        </label>
        <Field
          label="Start frame"
          type="number"
          min="0"
          value={startFrame}
          aria-invalid={!range.valid}
          aria-describedby={range.valid ? undefined : rangeMessageId}
          disabled={editingDisabled}
          onChange={(event) => setStartFrame(event.currentTarget.value)}
        />
        <Field
          label="End frame"
          type="number"
          min="1"
          value={endFrame}
          aria-invalid={!range.valid}
          aria-describedby={range.valid ? undefined : rangeMessageId}
          disabled={editingDisabled}
          onChange={(event) => setEndFrame(event.currentTarget.value)}
        />
        <label className="field" htmlFor={roleFieldId}>
          <span>Narrative role</span>
          <select
            id={roleFieldId}
            value={role}
            disabled={editingDisabled}
            onChange={(event) => setRole(NarrativeRoleSchema.parse(event.currentTarget.value))}
          >
            {roles.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
      </div>
      <Field
        label="Label"
        value={label}
        required
        hint={`${text.labelBytes}/200 UTF-8 bytes${label.trim().length === 0 ? " · Label cannot be blank." : ""}`}
        aria-invalid={label.trim().length === 0 || text.labelBytes > 200}
        disabled={editingDisabled}
        onChange={(event) => setLabel(event.currentTarget.value)}
      />
      <TextArea
        label="Note"
        rows={2}
        value={note}
        aria-invalid={text.noteBytes > 4000}
        disabled={editingDisabled}
        onChange={(event) => setNote(event.currentTarget.value)}
      />
      {clearsEvidence ? (
        <p className="moment-edit__warning">
          Saving this Source/range clears transcript evidence because the frozen selection is not
          fully contained.
        </p>
      ) : null}
      <MomentRangeStatus validation={range} id={rangeMessageId} />
      {moment.archived ? (
        <p className="moment-edit__warning" role="alert">
          This Moment was archived while editing. Restore it, then close and reopen this editor.
          Your draft remains visible until you close it.
        </p>
      ) : stale ? (
        <p className="moment-edit__warning" role="alert">
          This Moment changed from v{base.version} to v{moment.version}. Saving is blocked to
          protect your draft. Close and reopen this editor to load the latest version.
        </p>
      ) : null}
      <footer>
        <small>{text.noteBytes}/4000 UTF-8 bytes · saving returns review to draft</small>
        <div className="cluster">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            type="submit"
            busy={busy}
            disabled={!text.valid || !range.valid || !commandReady || moment.archived}
          >
            Save changes
          </Button>
        </div>
      </footer>
    </form>
  )
}
