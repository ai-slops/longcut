import { useCallback, useEffect, useRef, useState } from "react"
import type { AiProvider, AiRequest, AiRun, CodexStatus, ProviderStatus } from "../ai-domain"
import { isActiveRun } from "../ai-helpers"
import { ApiError, api } from "../api"
import type { Project } from "../domain"
import { AgentConnection } from "./AgentConnection"
import { AgentProviderSelector } from "./AgentProviderSelector"
import { AgentRequest } from "./AgentRequest"
import { TrackAiRuns } from "./TrackAiRuns"

type AgentPanelProps = {
  readonly project: Project
  readonly onRefreshProject: () => Promise<void>
  readonly onReview: () => void
}

function errorMessage(reason: unknown): string {
  if (reason instanceof ApiError) return `${reason.message} [${reason.code}]`
  if (reason instanceof Error) return reason.message
  return "An unexpected Codex request error occurred."
}

export function AgentPanel({ project, onRefreshProject, onReview }: AgentPanelProps) {
  const [connection, setConnection] = useState<CodexStatus | null>(null)
  const [provider, setProvider] = useState<AiProvider>("codex")
  const [providers, setProviders] = useState<readonly ProviderStatus[]>([])
  const [providersLoading, setProvidersLoading] = useState(true)
  const [runs, setRuns] = useState<readonly AiRun[]>([])
  const [connectionBusy, setConnectionBusy] = useState(false)
  const [runsLoading, setRunsLoading] = useState(true)
  const [submitting, setSubmitting] = useState(false)
  const [cancellingId, setCancellingId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [resetKey, setResetKey] = useState(0)
  const refreshedRuns = useRef(new Set<string>())
  const providerInitialized = useRef(false)

  const refreshConnection = useCallback(async (): Promise<void> => {
    setConnectionBusy(true)
    try {
      setConnection(await api.codexStatus())
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setConnectionBusy(false)
    }
  }, [])
  const refreshRuns = useCallback(async (): Promise<void> => {
    try {
      setRuns(await api.listAiRuns(project.id))
      setRunsLoading(false)
    } catch (reason) {
      setError(errorMessage(reason))
      setRunsLoading(false)
    }
  }, [project.id])
  const refreshProviders = useCallback(async (): Promise<void> => {
    try {
      const next = await api.aiProviders()
      setProviders(next)
      if (!providerInitialized.current) {
        providerInitialized.current = true
        if (next.find((item) => item.id === "openai-compatible")?.configured) {
          setProvider("openai-compatible")
        }
      }
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setProvidersLoading(false)
    }
  }, [])

  useEffect(() => {
    void refreshConnection()
    void refreshProviders()
    void refreshRuns()
  }, [refreshConnection, refreshProviders, refreshRuns])
  useEffect(() => {
    if (connection?.state !== "login_pending" && connection?.state !== "busy") return
    const interval = window.setInterval(() => void refreshConnection(), 2_000)
    return () => window.clearInterval(interval)
  }, [connection?.state, refreshConnection])
  useEffect(() => {
    if (!runs.some(isActiveRun)) return
    const interval = window.setInterval(() => void refreshRuns(), 2_000)
    return () => window.clearInterval(interval)
  }, [runs, refreshRuns])
  useEffect(() => {
    const published = runs.find(
      (run) => run.status === "ready" && run.change_set_id && !refreshedRuns.current.has(run.id),
    )
    if (!published) return
    refreshedRuns.current.add(published.id)
    void onRefreshProject()
  }, [runs, onRefreshProject])

  async function updateConnection(operation: () => Promise<CodexStatus>): Promise<void> {
    setConnectionBusy(true)
    setError(null)
    try {
      setConnection(await operation())
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setConnectionBusy(false)
    }
  }

  async function submit(request: AiRequest): Promise<boolean> {
    setSubmitting(true)
    setError(null)
    try {
      const run = await api.createAiRun(project.id, request)
      setRuns((current) => [run, ...current.filter((item) => item.id !== run.id)])
      return true
    } catch (reason) {
      setError(errorMessage(reason))
      return false
    } finally {
      setSubmitting(false)
    }
  }

  async function cancel(runId: string): Promise<void> {
    setCancellingId(runId)
    setError(null)
    try {
      const run = await api.cancelAiRun(project.id, runId)
      setRuns((current) => current.map((item) => (item.id === run.id ? run : item)))
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setCancellingId(null)
    }
  }

  return (
    <div className="workspace-panel agent-workspace">
      {error ? (
        <div className="agent-inline-error" role="alert">
          <span>{error}</span>
          <button type="button" onClick={() => setError(null)}>
            Dismiss
          </button>
        </div>
      ) : null}
      <aside className="agent-rail">
        <AgentProviderSelector
          provider={provider}
          providers={providers}
          loading={providersLoading}
          onChange={setProvider}
        />
        {provider === "codex" ? (
          <AgentConnection
            status={connection}
            loading={connectionBusy}
            onLogin={() => updateConnection(api.codexLogin)}
            onCancelLogin={() => updateConnection(api.cancelCodexLogin)}
            onLogout={() => updateConnection(api.codexLogout)}
            onRetry={refreshConnection}
          />
        ) : null}
        <TrackAiRuns
          runs={runs}
          loading={runsLoading}
          cancellingId={cancellingId}
          onCancel={cancel}
          onReview={onReview}
          onReset={() => setResetKey((value) => value + 1)}
        />
      </aside>
      <AgentRequest
        key={resetKey}
        project={project}
        provider={provider}
        providerStatus={providers.find((item) => item.id === provider) ?? null}
        connection={connection}
        submitting={submitting}
        onSubmit={submit}
      />
    </div>
  )
}
