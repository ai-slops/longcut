export type ProjectRevision = {
  readonly id: string
  readonly revision: string
}

export type ProjectRequestTicket = {
  readonly generation: number
  readonly expectedProjectId: string | null
}

export class ProjectRequestCoordinator {
  private generation = 0
  private active: ProjectRequestTicket | null = null

  begin(expectedProjectId: string | null): ProjectRequestTicket {
    const ticket = { generation: this.generation + 1, expectedProjectId }
    this.generation = ticket.generation
    this.active = ticket
    return ticket
  }

  invalidate(): void {
    this.generation += 1
    this.active = null
  }

  owns(ticket: ProjectRequestTicket): boolean {
    return this.active === ticket
  }

  release(ticket: ProjectRequestTicket): boolean {
    if (!this.owns(ticket)) return false
    this.active = null
    return true
  }
}

export function adoptProject<T extends ProjectRevision>(current: T | null, incoming: T): T | null {
  if (current === null || current.id !== incoming.id) return current
  return BigInt(incoming.revision) >= BigInt(current.revision) ? incoming : current
}
