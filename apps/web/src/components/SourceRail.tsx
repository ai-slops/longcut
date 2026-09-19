import { Camera, FileVideo2, Film, Music2, Upload } from "lucide-react"
import { useRef, useState } from "react"
import type { Project, Source } from "../domain"
import { Badge, Button, formatFrames, Panel } from "./Ui"

type SourceRailProps = {
  readonly project: Project
  readonly selectedId: string | null
  readonly busy: boolean
  readonly onSelect: (source: Source) => void
  readonly onImport: (file: File, role: Source["role"]) => Promise<void>
  readonly onDemo: () => Promise<void>
}

const roleIcon = { program: Film, primary: FileVideo2, camera: Camera, audio: Music2 } as const

export function SourceRail({
  project,
  selectedId,
  busy,
  onSelect,
  onImport,
  onDemo,
}: SourceRailProps) {
  const inputRef = useRef<HTMLInputElement>(null)
  const [importRole, setImportRole] = useState<Source["role"]>("program")
  return (
    <Panel title="Sources" className="source-rail" tools={<Badge>{project.sources.length}</Badge>}>
      <div className="source-actions">
        <label className="field">
          <span>Import as</span>
          <select
            value={importRole}
            onChange={(event) =>
              setImportRole(
                event.currentTarget.value === "primary"
                  ? "primary"
                  : event.currentTarget.value === "camera"
                    ? "camera"
                    : event.currentTarget.value === "audio"
                      ? "audio"
                      : "program",
              )
            }
          >
            <option value="program">Program</option>
            <option value="primary">Primary</option>
            <option value="camera">Camera</option>
            <option value="audio">Audio</option>
          </select>
        </label>
        <Button
          type="button"
          variant="secondary"
          onClick={() => inputRef.current?.click()}
          disabled={busy}
        >
          <Upload size={14} /> Import
        </Button>
        <Button type="button" variant="ghost" onClick={() => void onDemo()} disabled={busy}>
          Synthetic demo
        </Button>
        <input
          ref={inputRef}
          type="file"
          accept="video/*,audio/*"
          className="visually-hidden"
          onChange={(event) => {
            const file = event.currentTarget.files?.item(0)
            if (file) void onImport(file, importRole)
            event.currentTarget.value = ""
          }}
        />
      </div>
      <div className="source-list" role="listbox" aria-label="Project sources">
        {project.sources.length === 0 ? (
          <div className="panel-empty">
            <Film size={22} />
            <strong>No source media</strong>
            <p>Import a local file, or generate a visibly labelled test pattern.</p>
          </div>
        ) : (
          project.sources.map((source) => {
            const Icon = roleIcon[source.role]
            return (
              <button
                type="button"
                className={`source-card ${selectedId === source.id ? "selected" : ""}`}
                role="option"
                aria-selected={selectedId === source.id}
                key={source.id}
                onClick={() => onSelect(source)}
              >
                <span className="source-card__thumb">
                  {source.thumbnail_url ? (
                    <img src={source.thumbnail_url} width="192" height="108" alt="" />
                  ) : (
                    <Icon aria-hidden="true" size={20} />
                  )}
                  {source.name.toLowerCase().includes("synthetic") ? (
                    <Badge tone="warning">Synthetic</Badge>
                  ) : null}
                </span>
                <span className="source-card__meta">
                  <strong>{source.name}</strong>
                  <small>
                    {source.role} · {formatFrames(source.duration_frames, project.frame_rate)}
                  </small>
                </span>
                <Badge
                  tone={
                    source.status === "ready"
                      ? "success"
                      : source.status === "error"
                        ? "error"
                        : "warning"
                  }
                >
                  {source.status}
                </Badge>
              </button>
            )
          })
        )}
      </div>
    </Panel>
  )
}
