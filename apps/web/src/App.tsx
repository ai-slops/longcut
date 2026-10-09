import { useCallback, useEffect, useRef, useState } from "react"
import { ApiError, api } from "./api"
import { Editor } from "./components/Editor"
import { Home } from "./components/Home"
import { Showcase } from "./components/Showcase"
import type { Command, Project, ProjectSummary } from "./domain"
import { adoptProject, ProjectRequestCoordinator } from "./project-request"

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
  const projectRef = useRef<Project | null>(null)
  const requests = useRef(new ProjectRequestCoordinator())

  const replaceProject = useCallback((next: Project | null): void => {
    projectRef.current = next
    setProject(next)
  }, [])

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
    const projectId = project.id
    const id = window.setInterval(() => {
      void api
        .getProject(projectId)
        .then((incoming) => {
          const next = adoptProject(projectRef.current, incoming)
          if (next !== projectRef.current) replaceProject(next)
        })
        .catch((reason) => {
          if (projectRef.current?.id === projectId) report(reason)
        })
    }, 2000)
    return () => window.clearInterval(id)
  }, [project, replaceProject, report])

  const refreshOpenProject = useCallback(async (): Promise<void> => {
    if (!project) return
    const projectId = project.id
    try {
      const incoming = await api.getProject(projectId)
      const next = adoptProject(projectRef.current, incoming)
      if (next !== projectRef.current) replaceProject(next)
    } catch (reason) {
      if (projectRef.current?.id === projectId) report(reason)
    }
  }, [project, replaceProject, report])

  async function withBusy(
    operation: () => Promise<Project>,
    expectedProjectId: string | null,
  ): Promise<boolean> {
    const ticket = requests.current.begin(expectedProjectId)
    setBusy(true)
    setError(null)
    try {
      const incoming = await operation()
      if (!requests.current.owns(ticket)) return false
      if (expectedProjectId !== null && incoming.id !== expectedProjectId) return false
      const current = projectRef.current
      if (
        current !== null &&
        current.id === incoming.id &&
        BigInt(incoming.revision) < BigInt(current.revision)
      )
        return false
      replaceProject(incoming)
      return true
    } catch (reason) {
      if (!requests.current.owns(ticket)) return false
      report(reason)
      return false
    } finally {
      if (requests.current.release(ticket)) setBusy(false)
    }
  }

  if (!project) {
    return (
      <>
        <Home
          projects={projects}
          loading={busy}
          onOpen={(id) => void withBusy(() => api.getProject(id), id)}
          onCreate={async (title, frameRate) => {
            void (await withBusy(() => api.createProject(title, frameRate), null))
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
        requests.current.invalidate()
        setBusy(false)
        replaceProject(null)
        void refreshProjects()
      }}
      onCommand={(command: Command) => withBusy(() => api.command(project, command), project.id)}
      onRefreshProject={refreshOpenProject}
      onImport={async (file, role) => {
        void (await withBusy(() => api.importMedia(project, file, role), project.id))
      }}
      onSessionProject={(incoming) => {
        const next = adoptProject(projectRef.current, incoming)
        if (next !== projectRef.current) replaceProject(next)
      }}
      onDemo={async () => {
        void (await withBusy(() => api.createDemo(project), project.id))
      }}
      onRender={async (kind) => {
        void (await withBusy(async () => {
          await api.render(project, kind)
          return api.getProject(project.id)
        }, project.id))
      }}
    />
  )
}
