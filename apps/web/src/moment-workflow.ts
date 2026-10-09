import type { Command, Moment, MomentEvidenceSelection, Source } from "./domain"

export type MomentSelection = {
  readonly sourceId: string
  readonly inFrame: number
  readonly outFrame: number
  readonly evidence: MomentEvidenceSelection
  readonly previewText: string
}

type CurrentSelection = {
  readonly selection: MomentSelection | null
  readonly sourceId: string
  readonly inFrame: number
  readonly outFrame: number
}

export type MomentUpdate = {
  readonly sourceId?: string
  readonly startFrame?: string
  readonly endFrame?: string
  readonly label?: string
  readonly note?: string
  readonly narrativeRole?: Moment["narrative_role"]
}

type MomentEditDraft = {
  readonly sourceId: string
  readonly startFrame: string
  readonly endFrame: string
  readonly label: string
  readonly note: string
  readonly narrativeRole: Moment["narrative_role"]
}

type PreparedMomentUpdate =
  | {
      readonly kind: "ready"
      readonly command: Extract<Command, { readonly type: "update_moment" }>
    }
  | {
      readonly kind: "stale"
      readonly baseVersion: string
      readonly currentVersion: string
    }
  | { readonly kind: "invalid"; readonly reason: "invalid_format" }

export type MomentRangeValidation =
  | { readonly valid: true; readonly reason: null }
  | {
      readonly valid: false
      readonly reason: "invalid_format" | "non_positive" | "outside_source"
    }

export function evidenceForSelection({
  selection,
  sourceId,
  inFrame,
  outFrame,
}: CurrentSelection): MomentEvidenceSelection | null {
  if (
    selection?.sourceId !== sourceId ||
    selection.inFrame !== inFrame ||
    selection.outFrame !== outFrame
  )
    return null
  return selection.evidence
}

export function buildMomentUpdate(
  moment: Moment,
  update: MomentUpdate,
): Extract<Command, { readonly type: "update_moment" }> {
  const sourceId = update.sourceId ?? moment.source_id
  const startFrame = update.startFrame ?? moment.start_frame
  const endFrame = update.endFrame ?? moment.end_frame
  const frozenEvidence = moment.evidence[0]
  const containsEvidence =
    sourceId === moment.source_id &&
    frozenEvidence !== undefined &&
    startFrame === moment.start_frame &&
    endFrame === moment.end_frame
  return {
    type: "update_moment",
    moment_id: moment.id,
    expected_moment_version: moment.version,
    source_id: sourceId,
    start_frame: startFrame,
    end_frame: endFrame,
    label: update.label ?? moment.label,
    note: update.note ?? moment.note,
    narrative_role: update.narrativeRole ?? moment.narrative_role,
    evidence: containsEvidence
      ? {
          transcript_id: frozenEvidence.transcript_id,
          cue_ids: frozenEvidence.cue_ids,
          word_ids: frozenEvidence.word_ids,
        }
      : null,
  }
}

export function prepareMomentUpdate({
  base,
  currentVersion,
  draft,
}: {
  readonly base: Moment
  readonly currentVersion: string
  readonly draft: MomentEditDraft
}): PreparedMomentUpdate {
  if (base.version !== currentVersion) {
    return { kind: "stale", baseVersion: base.version, currentVersion }
  }
  const canonicalFrame = /^(0|[1-9]\d*)$/
  if (!canonicalFrame.test(draft.startFrame) || !canonicalFrame.test(draft.endFrame)) {
    return { kind: "invalid", reason: "invalid_format" }
  }
  return { kind: "ready", command: buildMomentUpdate(base, draft) }
}

export function validateMomentRange({
  startFrame,
  endFrame,
  durationFrames,
}: {
  readonly startFrame: string
  readonly endFrame: string
  readonly durationFrames: string
}): MomentRangeValidation {
  const canonicalFrame = /^(0|[1-9]\d*)$/
  if (
    !canonicalFrame.test(startFrame) ||
    !canonicalFrame.test(endFrame) ||
    !canonicalFrame.test(durationFrames)
  ) {
    return { valid: false, reason: "invalid_format" }
  }
  if (BigInt(endFrame) <= BigInt(startFrame)) {
    return { valid: false, reason: "non_positive" }
  }
  if (BigInt(endFrame) > BigInt(durationFrames)) {
    return { valid: false, reason: "outside_source" }
  }
  return { valid: true, reason: null }
}

export function canInsertMoment(moment: Moment, source: Source | undefined): boolean {
  return moment.review_status === "reviewed" && !moment.archived && sourceHasVideo(source)
}

export function sourceHasVideo(source: Source | undefined): source is Source {
  return source !== undefined && source.width > 0 && source.height > 0
}

export function momentReviewCommand(
  moment: Moment,
  reviewStatus: "reviewed" | "rejected",
): Command {
  return {
    type: "review_moment",
    moment_id: moment.id,
    expected_moment_version: moment.version,
    review_status: reviewStatus,
  }
}

export function momentArchiveCommand(moment: Moment): Command {
  return {
    type: moment.archived ? "restore_moment" : "archive_moment",
    moment_id: moment.id,
    expected_moment_version: moment.version,
  }
}

export function momentSceneCommand(moment: Moment): Command {
  return {
    type: "insert_scene_from_moment",
    moment_id: moment.id,
    expected_moment_version: moment.version,
    title: moment.label,
  }
}

export function validateMomentText(
  label: string,
  note: string,
): {
  readonly labelBytes: number
  readonly noteBytes: number
  readonly valid: boolean
} {
  const encoder = new TextEncoder()
  const labelBytes = encoder.encode(label).byteLength
  const noteBytes = encoder.encode(note).byteLength
  return {
    labelBytes,
    noteBytes,
    valid: label.trim().length > 0 && labelBytes <= 200 && noteBytes <= 4000,
  }
}

export function momentQuality(moment: Moment): {
  readonly reviewStatus: Moment["review_status"]
  readonly verification: "editorial_only"
  readonly transcriptOrigin: Moment["evidence"][number]["origin"] | null
  readonly transcriptVerification: "unverified" | "not_applicable"
} {
  const origin = moment.evidence[0]?.origin ?? null
  return {
    reviewStatus: moment.review_status,
    verification: "editorial_only",
    transcriptOrigin: origin,
    transcriptVerification: origin === null ? "not_applicable" : "unverified",
  }
}
