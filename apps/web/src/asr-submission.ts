import { type AsrJob, canResumeAsrJob } from "./asr-domain.ts"

export type AsrSubmissionAction = "start" | "cancel" | "resume"

export type AsrSubmissionScope = {
  readonly projectId: string
  readonly sourceId: string
}

export type AsrSubmissionTicket = {
  readonly generation: number
  readonly action: AsrSubmissionAction
  readonly scope: AsrSubmissionScope
}

export type AsrSubmissionRequest = {
  readonly action: AsrSubmissionAction
  readonly scope: AsrSubmissionScope
  readonly operation: (signal: AbortSignal) => Promise<AsrJob>
  readonly expectedJobId?: string
  readonly formatError?: (reason: unknown) => string
}

export type AsrSubmissionOutcome =
  | { readonly kind: "accepted"; readonly job: AsrJob; readonly released: boolean }
  | { readonly kind: "failed"; readonly reason: unknown; readonly released: boolean }
  | { readonly kind: "stale"; readonly released: boolean }

export class AsrActionContractError extends Error {
  readonly name = "AsrActionContractError"

  constructor(message: string) {
    super(message)
  }
}

export class AsrSubmissionCoordinator {
  private generation = 0
  private active: AsrSubmissionTicket | null = null

  get activeAction(): AsrSubmissionAction | null {
    return this.active?.action ?? null
  }

  begin(scope: AsrSubmissionScope, action: AsrSubmissionAction): AsrSubmissionTicket | null {
    if (this.active) return null
    const ticket = { generation: this.generation + 1, action, scope }
    this.generation = ticket.generation
    this.active = ticket
    return ticket
  }

  invalidate(): void {
    this.generation += 1
    this.active = null
  }

  async settle(
    ticket: AsrSubmissionTicket,
    response: Promise<AsrJob>,
    expectedJobId?: string,
  ): Promise<AsrSubmissionOutcome> {
    try {
      const job = await response
      if (!this.owns(ticket)) return { kind: "stale", released: false }
      if (job.project_id !== ticket.scope.projectId || job.source_id !== ticket.scope.sourceId) {
        throw new AsrActionContractError(
          `ASR ${ticket.action} returned a job for another project or Source.`,
        )
      }
      if (expectedJobId && job.id !== expectedJobId) {
        throw new AsrActionContractError(`ASR ${ticket.action} returned a different job.`)
      }
      return { kind: "accepted", job, released: this.release(ticket) }
    } catch (reason) {
      if (!this.owns(ticket)) return { kind: "stale", released: false }
      return { kind: "failed", reason, released: this.release(ticket) }
    }
  }

  private owns(ticket: AsrSubmissionTicket): boolean {
    return this.active === ticket
  }

  private release(ticket: AsrSubmissionTicket): boolean {
    if (!this.owns(ticket)) return false
    this.active = null
    return true
  }
}

export function canSubmitAsrResume(
  job: AsrJob,
  runtimeAvailable: boolean,
  submitting: boolean,
): boolean {
  return runtimeAvailable && !submitting && canResumeAsrJob(job)
}
