import type { StoryVersionResponse } from "./story-domain.ts"

export type StoryVersionIdentity = {
  readonly projectId: string
  readonly version: string
  readonly key: string
}

export type StoryVersionTicket = {
  readonly identity: StoryVersionIdentity
  readonly signal: AbortSignal
}

export function storyVersionIdentity(projectId: string, version: string): StoryVersionIdentity {
  return { projectId, version, key: `${projectId}:${version}` }
}

export function storyVersionResponseMatchesIdentity(
  identity: StoryVersionIdentity,
  response: StoryVersionResponse,
): boolean {
  return (
    response.project_id === identity.projectId &&
    response.version === identity.version &&
    response.snapshot.version === identity.version
  )
}

export class StoryVersionRequestCoordinator {
  private active: StoryVersionTicket | null = null
  private controller: AbortController | null = null

  begin(identity: StoryVersionIdentity): StoryVersionTicket {
    this.controller?.abort()
    this.controller = new AbortController()
    const ticket = { identity, signal: this.controller.signal }
    this.active = ticket
    return ticket
  }

  invalidate(): void {
    this.controller?.abort()
    this.controller = null
    this.active = null
  }

  owns(ticket: StoryVersionTicket): boolean {
    return this.active === ticket && !ticket.signal.aborted
  }

  release(ticket: StoryVersionTicket): boolean {
    if (!this.owns(ticket)) return false
    this.controller = null
    this.active = null
    return true
  }
}
