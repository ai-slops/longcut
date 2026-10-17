export type RuntimeRequestIdentity = {
  readonly projectId: string
  readonly runId: string
}

export type RuntimeRequestTicket = {
  readonly identity: RuntimeRequestIdentity
  readonly signal: AbortSignal
}

export class RuntimeRequestCoordinator {
  private active: RuntimeRequestTicket | null = null
  private controller: AbortController | null = null

  begin(identity: RuntimeRequestIdentity): RuntimeRequestTicket {
    this.controller?.abort()
    const controller = new AbortController()
    const ticket = { identity, signal: controller.signal }
    this.controller = controller
    this.active = ticket
    return ticket
  }

  owns(ticket: RuntimeRequestTicket): boolean {
    return this.active === ticket && !ticket.signal.aborted
  }

  invalidate(): void {
    this.controller?.abort()
    this.controller = null
    this.active = null
  }
}
