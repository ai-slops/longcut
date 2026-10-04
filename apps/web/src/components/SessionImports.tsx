import { FolderInput } from "lucide-react"
import { useCallback, useEffect, useRef, useState } from "react"
import { ApiError, api } from "../api"
import type { Project } from "../domain"
import type { SessionImportJob } from "../session-domain"
import { PackageSelectionError, preparePackageFiles } from "../session-files"
import { SessionSummary } from "./SessionSummary"
import { Badge, Button } from "./Ui"

type SessionImportsProps = {
  readonly project: Project
  readonly onProject: (project: Project) => void
}

function errorMessage(reason: unknown): string {
  if (reason instanceof ApiError) return `${reason.message} [${reason.code}]`
  if (reason instanceof Error) return reason.message
  return "An unexpected session import error occurred."
}

function replaceJob(jobs: readonly SessionImportJob[], incoming: SessionImportJob) {
  return [incoming, ...jobs.filter((job) => job.id !== incoming.id)]
}

export function SessionImports({ project, onProject }: SessionImportsProps) {
  const directoryInput = useRef<HTMLInputElement>(null)
  const [jobs, setJobs] = useState<readonly SessionImportJob[]>([])
  const [busyId, setBusyId] = useState<string | null>(null)
  const [acknowledged, setAcknowledged] = useState<readonly string[]>([])
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    directoryInput.current?.setAttribute("webkitdirectory", "")
  }, [])

  const refreshJobs = useCallback(async (): Promise<void> => {
    try {
      setJobs(await api.listSessionImports(project.id))
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }, [project.id])

  useEffect(() => {
    void refreshJobs()
  }, [refreshJobs])

  useEffect(() => {
    const active = jobs.some((job) => job.status === "validating" || job.status === "publishing")
    if (!active) return
    const interval = window.setInterval(() => void refreshJobs(), 2000)
    return () => window.clearInterval(interval)
  }, [jobs, refreshJobs])

  async function upload(files: FileList | null): Promise<void> {
    setError(null)
    try {
      const selected = preparePackageFiles(files ? Array.from(files) : [])
      setBusyId("upload")
      const job = await api.createSessionImport(project.id, selected)
      setJobs((current) => replaceJob(current, job))
    } catch (reason) {
      if (reason instanceof PackageSelectionError || reason instanceof Error) {
        setError(errorMessage(reason))
        return
      }
      throw reason
    } finally {
      setBusyId(null)
    }
  }

  async function confirm(job: SessionImportJob): Promise<void> {
    const allowDegraded = job.session?.conformance === "degraded"
    setBusyId(job.id)
    setError(null)
    try {
      onProject(await api.confirmSessionImport(project, job.id, allowDegraded))
      await refreshJobs()
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setBusyId(null)
    }
  }

  async function cancel(job: SessionImportJob): Promise<void> {
    setBusyId(job.id)
    setError(null)
    try {
      const cancelled = await api.cancelSessionImport(project.id, job.id)
      setJobs((current) => replaceJob(current, cancelled))
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setBusyId(null)
    }
  }

  return (
    <section className="session-imports" aria-labelledby="session-import-title">
      <header className="session-imports__header">
        <div>
          <p className="eyebrow">Completed package</p>
          <h2 id="session-import-title">Session imports</h2>
          <p>Upload a complete package directory, review verification, then commit once.</p>
        </div>
        <Button
          type="button"
          variant="secondary"
          busy={busyId === "upload"}
          onClick={() => directoryInput.current?.click()}
        >
          <FolderInput size={15} /> Select directory
        </Button>
        <input
          ref={directoryInput}
          className="visually-hidden"
          type="file"
          multiple
          onChange={(event) => {
            void upload(event.currentTarget.files)
            event.currentTarget.value = ""
          }}
        />
      </header>
      {error ? (
        <p className="session-imports__error" role="alert">
          {error}
        </p>
      ) : null}
      <div className="session-imports__jobs">
        {jobs.map((job) => {
          const degraded = job.session?.conformance === "degraded"
          const canConfirm = job.status === "prepared" && job.session?.conformance !== "rejected"
          return (
            <article className="session-job" key={job.id}>
              <header>
                <div>
                  <strong>{job.session?.title ?? "Validating package"}</strong>
                  <small>{new Date(job.created_at).toLocaleString()}</small>
                </div>
                <Badge
                  tone={
                    job.status === "failed"
                      ? "error"
                      : job.status === "confirmed"
                        ? "success"
                        : "warning"
                  }
                >
                  {job.status}
                </Badge>
              </header>
              {job.error ? <p className="error-copy">{job.error}</p> : null}
              {job.session ? (
                <SessionSummary
                  projectId={project.id}
                  session={job.session}
                  assetsDownloadable={false}
                />
              ) : null}
              {degraded && job.status === "prepared" ? (
                <label className="check-field session-job__acknowledgement">
                  <input
                    type="checkbox"
                    checked={acknowledged.includes(job.id)}
                    onChange={(event) => {
                      const checked = event.currentTarget.checked
                      setAcknowledged((current) =>
                        checked ? [...current, job.id] : current.filter((id) => id !== job.id),
                      )
                    }}
                  />
                  I acknowledge that unsupported or offline sources will not become editable media.
                </label>
              ) : null}
              {job.status === "validating" ||
              job.status === "prepared" ||
              job.status === "publishing" ? (
                <footer>
                  <Button
                    type="button"
                    variant="ghost"
                    busy={busyId === job.id}
                    disabled={job.status === "publishing"}
                    onClick={() => void cancel(job)}
                  >
                    Cancel
                  </Button>
                  {canConfirm ? (
                    <Button
                      type="button"
                      variant="primary"
                      busy={busyId === job.id}
                      disabled={degraded && !acknowledged.includes(job.id)}
                      onClick={() => void confirm(job)}
                    >
                      Confirm import
                    </Button>
                  ) : null}
                </footer>
              ) : null}
            </article>
          )
        })}
      </div>
      <section className="source-sessions" aria-labelledby="source-sessions-title">
        <header>
          <div>
            <p className="eyebrow">Immutable provenance</p>
            <h3 id="source-sessions-title">Imported source sessions</h3>
          </div>
          <Badge>{project.source_sessions.length}</Badge>
        </header>
        {project.source_sessions.length === 0 ? (
          <p className="muted">No completed package has been committed to this project.</p>
        ) : (
          project.source_sessions.map((session) => (
            <SessionSummary
              key={session.id}
              projectId={project.id}
              session={session}
              assetsDownloadable
            />
          ))
        )}
      </section>
    </section>
  )
}
