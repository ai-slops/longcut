import { useCallback, useEffect, useState } from "react"
import { ApiError, api } from "./api"
import { Editor } from "./components/Editor"
import { Home } from "./components/Home"
import { Showcase } from "./components/Showcase"
import type { Command, Project, ProjectSummary } from "./domain"

export function App() {
  return new URLSearchParams(window.location.search).has("showcase") ? (
    <Showcase />
  ) : (
    <Application />
  )
}

function Application() {
  const [projects, setProjects] = useState<readonly ProjectSummary[]>([])
  const [project, setProject] = useState<Project | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const report = useCallback((reason: unknown): void => {
    if (reason instanceof ApiError) setError(`${reason.message} [${reason.code}]`)
    else if (reason instanceof Error) setError(reason.message)
    else setError("An unexpected request error occurred.")
  }, [])

  const refreshProjects = useCallback(async (): Promise<void> => {
    try {
      setProjects(await api.listProjects())
    } catch (reason) {
      report(reason)
    }
  }, [report])

  useEffect(() => {
    void refreshProjects()
  }, [refreshProjects])

  useEffect(() => {
    const pending =
      project?.sources.some((source) => source.status === "processing") ||
      project?.renders.some((render) => render.status === "queued" || render.status === "running")
    if (!project || !pending) return
    const id = window.setInterval(() => {
      void api
        .getProject(project.id)
        .then((incoming) =>
          setProject((current) =>
            current &&
            current.id === incoming.id &&
            Number(current.revision) > Number(incoming.revision)
              ? current
              : incoming,
          ),
        )
        .catch(report)
    }, 2000)
    return () => window.clearInterval(id)
  }, [project, report])

  const refreshOpenProject = useCallback(async (): Promise<void> => {
    if (!project) return
    try {
      const incoming = await api.getProject(project.id)
      setProject((current) =>
        current &&
        current.id === incoming.id &&
        Number(current.revision) > Number(incoming.revision)
          ? current
          : incoming,
      )
    } catch (reason) {
      report(reason)
    }
  }, [project, report])

  async function withBusy(operation: () => Promise<Project>): Promise<boolean> {
    setBusy(true)
    setError(null)
    try {
      setProject(await operation())
      return true
    } catch (reason) {
      report(reason)
      return false
    } finally {
      setBusy(false)
    }
  }

  if (!project) {
    return (
      <>
        <Home
          projects={projects}
          loading={busy}
          onOpen={(id) => void withBusy(() => api.getProject(id))}
          onCreate={async (title, frameRate) => {
            void (await withBusy(() => api.createProject(title, frameRate)))
          }}
        />
        {error ? (
          <div className="home-error" role="alert">
            {error}
            <button type="button" onClick={() => setError(null)}>
              Dismiss
            </button>
          </div>
        ) : null}
      </>
    )
  }

  return (
    <Editor
      project={project}
      busy={busy}
      error={error}
      onClearError={() => setError(null)}
      onBack={() => {
        setProject(null)
        void refreshProjects()
      }}
      onCommand={(command: Command) => withBusy(() => api.command(project, command))}
      onRefreshProject={refreshOpenProject}
      onImport={async (file, role) => {
        void (await withBusy(() => api.importMedia(project, file, role)))
      }}
      onSessionProject={(incoming) =>
        setProject((current) =>
          current?.id === incoming.id && BigInt(current.revision) <= BigInt(incoming.revision)
            ? incoming
            : current,
        )
      }
      onDemo={async () => {
        void (await withBusy(() => api.createDemo(project)))
      }}
      onRender={async (kind) => {
        void (await withBusy(async () => {
          await api.render(project, kind)
          return api.getProject(project.id)
        }))
      }}
    />
  )
}
