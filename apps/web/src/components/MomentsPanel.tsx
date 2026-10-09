import { Bookmark } from "lucide-react"
import { useMemo, useState } from "react"
import { z } from "zod"
import type { Command, Moment, Project } from "../domain"
import { MomentCard } from "./MomentCard"
import { Badge, Button, Panel } from "./Ui"

type MomentsPanelProps = {
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onReveal: (moment: Moment) => void
}

const pageSize = 40
const filters = ["all", "draft", "reviewed", "rejected", "archived"] as const
type MomentFilter = (typeof filters)[number]
const MomentFilterSchema = z.enum(filters)

export function MomentsPanel({ project, busy, onCommand, onReveal }: MomentsPanelProps) {
  const [filter, setFilter] = useState<MomentFilter>("all")
  const [query, setQuery] = useState("")
  const [limit, setLimit] = useState(pageSize)
  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase()
    return project.moments.filter((moment) => {
      const statusMatches =
        filter === "all"
          ? true
          : filter === "archived"
            ? moment.archived
            : !moment.archived && moment.review_status === filter
      const textMatches =
        !needle || `${moment.label}\n${moment.note}`.toLocaleLowerCase().includes(needle)
      return statusMatches && textMatches
    })
  }, [filter, project.moments, query])
  const visible = filtered.slice(0, limit)

  return (
    <Panel
      title="Moments"
      className="moments-panel"
      tools={<Badge tone="violet">{project.moments.length} saved</Badge>}
    >
      <div className="moments-panel__tools">
        <label className="field">
          <span>Filter</span>
          <select
            value={filter}
            onChange={(event) => {
              setFilter(MomentFilterSchema.parse(event.currentTarget.value))
              setLimit(pageSize)
            }}
          >
            {filters.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Search label or note</span>
          <input
            type="search"
            value={query}
            onChange={(event) => {
              setQuery(event.currentTarget.value)
              setLimit(pageSize)
            }}
          />
        </label>
      </div>
      <p className="panel-intro">
        Curated Source ranges stay independent from scene order until you explicitly add a reviewed
        visual Moment.
      </p>
      {visible.length === 0 ? (
        <div className="panel-empty">
          <Bookmark size={22} />
          <strong>No matching Moments</strong>
          <p>Save a Source range in Edit or change this filter.</p>
        </div>
      ) : (
        <div className="moment-list">
          {visible.map((moment) => (
            <MomentCard
              key={moment.id}
              project={project}
              moment={moment}
              busy={busy}
              onCommand={onCommand}
              onReveal={onReveal}
            />
          ))}
        </div>
      )}
      {visible.length < filtered.length ? (
        <Button
          type="button"
          variant="ghost"
          className="moments-panel__more"
          onClick={() => setLimit((value) => value + pageSize)}
        >
          Show 40 more
        </Button>
      ) : null}
    </Panel>
  )
}
