import { Button } from "./Ui"

type RangeControlsProps = {
  readonly inFrame: number
  readonly outFrame: number
  readonly current: number
  readonly onIn: (value: number) => void
  readonly onOut: (value: number) => void
  readonly onAdd: () => void
  readonly onSaveMoment: () => void
  readonly disabled: boolean
  readonly sceneDisabled: boolean
}

export function RangeControls({
  inFrame,
  outFrame,
  current,
  onIn,
  onOut,
  onAdd,
  onSaveMoment,
  disabled,
  sceneDisabled,
}: RangeControlsProps) {
  return (
    <div className="range-controls">
      <button type="button" onClick={() => onIn(current)}>
        Mark In <kbd>I</kbd>
      </button>
      <code>{inFrame}</code>
      <span>→</span>
      <code>{outFrame}</code>
      <button type="button" onClick={() => onOut(current)}>
        Mark Out <kbd>O</kbd>
      </button>
      <Button
        type="button"
        variant="primary"
        onClick={onAdd}
        disabled={disabled || sceneDisabled || outFrame <= inFrame}
      >
        Add scene
      </Button>
      <Button
        type="button"
        variant="secondary"
        onClick={onSaveMoment}
        disabled={disabled || outFrame <= inFrame}
      >
        Save moment
      </Button>
    </div>
  )
}
