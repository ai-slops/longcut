export type MomentVersionIdentity = {
  readonly projectId: string
  readonly momentId: string
  readonly version: string
  readonly key: string
}

export type MomentVersionFetch = MomentVersionIdentity & {
  readonly signal: AbortSignal
}

export type MomentVersionTicket = {
  readonly generation: number
  readonly identity: MomentVersionIdentity
  readonly signal: AbortSignal
}

export function momentVersionIdentity(
  projectId: string,
  momentId: string,
  version: string,
): MomentVersionIdentity {
  return { projectId, momentId, version, key: `${projectId}:${momentId}:${version}` }
}

export function snapshotMatchesIdentity(
  identity: MomentVersionIdentity,
  snapshot: { readonly id: string; readonly version: string },
): boolean {
  return snapshot.id === identity.momentId && snapshot.version === identity.version
}

export function versionResponseMatchesIdentity(
  identity: MomentVersionIdentity,
  response: {
    readonly project_id: string
    readonly moment_id: string
    readonly version: string
    readonly snapshot: { readonly id: string; readonly version: string }
  },
): boolean {
  return (
    response.project_id === identity.projectId &&
    response.moment_id === identity.momentId &&
    response.version === identity.version &&
    snapshotMatchesIdentity(identity, response.snapshot)
  )
}

export class MomentVersionRequestCoordinator {
  private generation = 0
  private active: MomentVersionTicket | null = null
  private controller: AbortController | null = null

  begin(identity: MomentVersionIdentity): MomentVersionTicket {
    this.controller?.abort()
    this.controller = new AbortController()
    const ticket = {
      generation: this.generation + 1,
      identity,
      signal: this.controller.signal,
    }
    this.generation = ticket.generation
    this.active = ticket
    return ticket
  }

  invalidate(): void {
    this.controller?.abort()
    this.controller = null
    this.active = null
    this.generation += 1
  }

  owns(ticket: MomentVersionTicket): boolean {
    return this.active === ticket && !ticket.signal.aborted
  }

  release(ticket: MomentVersionTicket): boolean {
    if (!this.owns(ticket)) return false
    this.active = null
    this.controller = null
    return true
  }
}
