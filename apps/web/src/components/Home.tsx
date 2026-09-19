import { ArrowRight, Clapperboard, FolderOpen, Plus } from "lucide-react"
import { type FormEvent, useState } from "react"
import type { ProjectSummary } from "../domain"
import { Button, Field } from "./Ui"

type HomeProps = {
  readonly projects: readonly ProjectSummary[]
  readonly loading: boolean
  readonly onCreate: (title: string, frameRate: 30 | 60) => Promise<void>
  readonly onOpen: (id: string) => void
}

export function Home({ projects, loading, onCreate, onOpen }: HomeProps) {
  const [title, setTitle] = useState("")
  const [frameRate, setFrameRate] = useState<30 | 60>(30)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    const cleanTitle = title.trim()
    if (cleanTitle.length === 0) return
    await onCreate(cleanTitle, frameRate)
  }

  return (
    <main className="home">
      <header className="home__brand">
        <LongcutMark />
        <span>LONGCUT</span>
        <small>FRAME-TRUE EDITOR</small>
      </header>
      <section className="home__hero">
        <div>
          <p className="eyebrow">A deliberate cut</p>
          <h1>
            Long recordings.
            <br />
            <em>Short decisions.</em>
          </h1>
          <p>
            Build scenes from exact source ranges, keep review evidence attached, and render from
            immutable originals.
          </p>
        </div>
        <form className="create-card" onSubmit={(event) => void submit(event)}>
          <div className="create-card__head">
            <Plus size={18} />
            <div>
              <h2>New project</h2>
              <p>Start with an empty, local edit.</p>
            </div>
          </div>
          <Field
            label="Project title"
            value={title}
            onChange={(event) => setTitle(event.currentTarget.value)}
            placeholder="Interview assembly"
            autoFocus
          />
          <fieldset>
            <legend>Frame rate</legend>
            <div className="segmented">
              {[30, 60].map((rate) => (
                <button
                  type="button"
                  className={frameRate === rate ? "active" : ""}
                  key={rate}
                  onClick={() => setFrameRate(rate === 30 ? 30 : 60)}
                >
                  {rate} fps
                </button>
              ))}
            </div>
          </fieldset>
          <Button type="submit" variant="primary" busy={loading}>
            Create project <ArrowRight size={15} />
          </Button>
        </form>
      </section>
      <section className="recent">
        <div className="recent__heading">
          <div>
            <p className="eyebrow">Workspace</p>
            <h2>Recent projects</h2>
          </div>
          <span>{projects.length} local</span>
        </div>
        {projects.length === 0 ? (
          <div className="empty-list">
            <Clapperboard size={24} />
            <div>
              <strong>No projects yet</strong>
              <p>Create one above, then import media or generate the labelled synthetic demo.</p>
            </div>
          </div>
        ) : (
          <div className="project-grid">
            {projects.map((project) => (
              <button
                className="project-card"
                type="button"
                key={project.id}
                onClick={() => onOpen(project.id)}
              >
                <span className="project-card__visual">
                  <FolderOpen size={24} />
                </span>
                <span>
                  <strong>{project.title}</strong>
                  <small>
                    Revision {project.revision} · {new Date(project.updated_at).toLocaleString()}
                  </small>
                </span>
                <ArrowRight size={16} />
              </button>
            ))}
          </div>
        )}
      </section>
    </main>
  )
}

function LongcutMark() {
  return (
    <span className="brand-mark" aria-hidden="true">
      <i />
      <i />
      <i />
    </span>
  )
}
