import type { MomentRangeValidation } from "../moment-workflow"

const rangeMessages: Record<Exclude<MomentRangeValidation["reason"], null>, string> = {
  invalid_format: "Use whole, non-negative frame numbers.",
  non_positive: "End frame must be after start frame.",
  outside_source: "Range must stay within the selected Source.",
}

export function MomentRangeStatus({
  validation,
  id,
}: {
  readonly validation: MomentRangeValidation
  readonly id?: string
}) {
  if (validation.valid) return null
  return (
    <p className="moment-edit__warning" id={id} role="alert">
      {rangeMessages[validation.reason]}
    </p>
  )
}
