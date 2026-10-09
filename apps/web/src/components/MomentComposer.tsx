import { BookmarkPlus, X } from "lucide-react"
import { type FormEvent, useState } from "react"
import { type Command, type Moment, NarrativeRoleSchema, type Source } from "../domain"
import {
  evidenceForSelection,
  type MomentSelection,
  validateMomentRange,
  validateMomentText,
} from "../moment-workflow"
import { MomentRangeStatus } from "./MomentRangeStatus"
import { Badge, Button, Field, TextArea } from "./Ui"

type MomentComposerProps = {
  readonly source: Source
  readonly inFrame: number
  readonly outFrame: number
  readonly selection: MomentSelection | null
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

export function MomentComposer({
  source,
  inFrame,
  outFrame,
  selection,
  busy,
  onCommand,
  onClose,
}: MomentComposerProps) {
  const [label, setLabel] = useState("")
  const [note, setNote] = useState("")
  const [role, setRole] = useState<Moment["narrative_role"]>("other")
  const text = validateMomentText(label, note)
  const range = validateMomentRange({
    startFrame: String(inFrame),
    endFrame: String(outFrame),
    durationFrames: source.duration_frames,
  })
  const selectedEvidence = evidenceForSelection({
    selection,
    sourceId: source.id,
    inFrame,
    outFrame,
  })

  async function save(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (!text.valid || !range.valid) return
    const saved = await onCommand({
      type: "create_moment",
      source_id: source.id,
      start_frame: String(inFrame),
      end_frame: String(outFrame),
      label,
      note,
      narrative_role: role,
      evidence: selectedEvidence,
    })
    if (saved) onClose()
  }

  return (
    <form className="moment-composer" onSubmit={(event) => void save(event)}>
      <header>
        <div>
          <p className="eyebrow">
            Source range {inFrame}–{outFrame}
          </p>
          <h3>Save moment</h3>
        </div>
        <button type="button" aria-label="Close Moment composer" onClick={onClose} disabled={busy}>
          <X size={15} />
        </button>
      </header>
      {selectedEvidence ? (
        <div className="moment-composer__evidence">
          <Badge tone="violet">Transcript selection</Badge>
          <q>{selection?.previewText}</q>
          <small>
            IDs will be resolved and frozen by the backend; this preview is not submitted.
          </small>
        </div>
      ) : (
        <p className="moment-composer__manual">
          Manual Source range · transcript evidence is optional.
        </p>
      )}
      <div className="moment-composer__fields">
        <Field
          label="Label"
          value={label}
          required
          aria-invalid={label.trim().length === 0 || text.labelBytes > 200}
          hint={`${text.labelBytes}/200 UTF-8 bytes${label.trim().length === 0 ? " · Label cannot be blank." : ""}`}
          onChange={(event) => setLabel(event.currentTarget.value)}
        />
        <label className="field">
          <span>Narrative role</span>
          <select
            value={role}
            onChange={(event) => setRole(NarrativeRoleSchema.parse(event.currentTarget.value))}
          >
            {roles.map((item) => (
              <option value={item} key={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
      </div>
      <TextArea
        label="Note"
        value={note}
        rows={2}
        aria-invalid={text.noteBytes > 4000}
        onChange={(event) => setNote(event.currentTarget.value)}
      />
      <MomentRangeStatus validation={range} />
      <footer>
        <small>{text.noteBytes}/4000 UTF-8 bytes</small>
        <Button type="submit" variant="primary" busy={busy} disabled={!text.valid || !range.valid}>
          <BookmarkPlus size={14} /> Save moment
        </Button>
      </footer>
    </form>
  )
}
