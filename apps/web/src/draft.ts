export type Draft = {
  readonly entity: string
  readonly baseline: string
  readonly value: string
}

export function syncDraft(
  state: Draft,
  incoming: { readonly entity: string; readonly value: string },
): Draft {
  if (state.entity === incoming.entity && state.baseline === incoming.value) return state
  return {
    entity: incoming.entity,
    baseline: incoming.value,
    value:
      state.entity !== incoming.entity || state.value === state.baseline
        ? incoming.value
        : state.value,
  }
}
