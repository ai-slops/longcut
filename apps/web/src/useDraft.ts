import { useState } from "react"
import { syncDraft } from "./draft"

export function useDraft(value: string, entity: string) {
  const [state, setState] = useState({ value, baseline: value, entity })
  const synchronized = syncDraft(state, { value, entity })
  if (synchronized !== state) setState(synchronized)
  return [
    synchronized.value,
    (next: string) => setState((current) => ({ ...current, value: next })),
  ] as const
}
