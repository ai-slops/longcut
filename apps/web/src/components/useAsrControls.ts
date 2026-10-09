import { useCallback, useEffect, useRef, useState } from "react"
import { ApiError } from "../api"
import { asrApi } from "../asr-api"
import {
  type AsrJob,
  type AsrLanguage,
  type AsrRuntimeStatus,
  asrJobBelongsToSource,
  asrJobStorageKey,
  asrRequestMatches,
  canPollAsrJob,
  canRetryAsrPoll,
  isActiveAsrJob,
} from "../asr-domain"
import type { Source } from "../domain"

type UseAsrControlsInput = {
  readonly projectId: string
  readonly source: Source | null
  readonly onTranscriptReady: () => void
}

type AsrController = {
  readonly runtime: AsrRuntimeStatus | null
  readonly job: AsrJob | null
  readonly language: AsrLanguage
  readonly checking: boolean
  readonly submitting: boolean
  readonly pollFailed: boolean
  readonly error: string | null
  readonly setLanguage: (language: AsrLanguage) => void
  readonly refreshRuntime: () => Promise<void>
  readonly start: () => Promise<void>
  readonly cancel: () => Promise<void>
  readonly retryPoll: () => void
}

function errorMessage(reason: unknown): string {
  if (reason instanceof ApiError) return `${reason.message} [${reason.code}]`
  if (reason instanceof Error) return reason.message
  return "An unexpected local ASR error occurred."
}

export function useAsrControls({
  projectId,
  source,
  onTranscriptReady,
}: UseAsrControlsInput): AsrController {
  const sourceId = source?.id
  const [runtime, setRuntime] = useState<AsrRuntimeStatus | null>(null)
  const [language, setLanguage] = useState<AsrLanguage>("ko")
  const [job, setJob] = useState<AsrJob | null>(null)
  const [checking, setChecking] = useState(true)
  const [submitting, setSubmitting] = useState(false)
  const [pollFailed, setPollFailed] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const runtimeRequest = useRef<AbortController | null>(null)
  const jobRequest = useRef<AbortController | null>(null)
  const requestVersion = useRef(0)
  const announcedTranscript = useRef<string | null>(null)

  const refreshRuntime = useCallback(async (): Promise<void> => {
    runtimeRequest.current?.abort()
    const controller = new AbortController()
    runtimeRequest.current = controller
    setChecking(true)
    setError(null)
    try {
      setRuntime(await asrApi.status(controller.signal))
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") return
      setRuntime(null)
      setError(errorMessage(reason))
    } finally {
      if (!controller.signal.aborted) setChecking(false)
    }
  }, [])

  useEffect(() => {
    void refreshRuntime()
    return () => runtimeRequest.current?.abort()
  }, [refreshRuntime])

  useEffect(() => {
    requestVersion.current += 1
    jobRequest.current?.abort()
    setJob(null)
    setSubmitting(false)
    setPollFailed(false)
    setError(null)
    announcedTranscript.current = null
    if (!sourceId) return
    const storageKey = asrJobStorageKey(projectId, sourceId)
    const jobId = localStorage.getItem(storageKey)
    if (!jobId) return
    const controller = new AbortController()
    const responseVersion = requestVersion.current
    jobRequest.current = controller
    void asrApi
      .get({ projectId, sourceId, jobId, signal: controller.signal })
      .then((recovered) => {
        if (
          asrRequestMatches(requestVersion.current, responseVersion) &&
          asrJobBelongsToSource(recovered, projectId, sourceId)
        ) {
          setJob(recovered)
          setPollFailed(false)
        }
      })
      .catch((reason: unknown) => {
        if (reason instanceof DOMException && reason.name === "AbortError") return
        if (!asrRequestMatches(requestVersion.current, responseVersion)) return
        if (reason instanceof ApiError && reason.status === 404) {
          localStorage.removeItem(storageKey)
          setError("The previous local ASR job is no longer available.")
          return
        }
        setError(errorMessage(reason))
      })
    return () => {
      controller.abort()
      requestVersion.current += 1
    }
  }, [projectId, sourceId])

  useEffect(() => {
    if (!sourceId || !job || !canPollAsrJob(job, pollFailed, submitting)) return
    const controller = new AbortController()
    const responseVersion = requestVersion.current
    const timer = window.setTimeout(() => {
      void asrApi
        .get({ projectId, sourceId, jobId: job.id, signal: controller.signal })
        .then((incoming) => {
          if (
            asrRequestMatches(requestVersion.current, responseVersion) &&
            asrJobBelongsToSource(incoming, projectId, sourceId)
          ) {
            setJob(incoming)
            setPollFailed(false)
            setError(null)
          }
        })
        .catch((reason: unknown) => {
          if (reason instanceof DOMException && reason.name === "AbortError") return
          if (asrRequestMatches(requestVersion.current, responseVersion)) {
            setPollFailed(true)
            setError(errorMessage(reason))
          }
        })
    }, 1_000)
    return () => {
      window.clearTimeout(timer)
      controller.abort()
    }
  }, [job, pollFailed, projectId, sourceId, submitting])

  useEffect(() => {
    if (job?.status !== "ready" || !job.transcript_id) return
    if (announcedTranscript.current === job.transcript_id) return
    announcedTranscript.current = job.transcript_id
    onTranscriptReady()
  }, [job, onTranscriptReady])

  async function start(): Promise<void> {
    if (!source || !runtime?.available) return
    jobRequest.current?.abort()
    const controller = new AbortController()
    const responseVersion = requestVersion.current + 1
    requestVersion.current = responseVersion
    jobRequest.current = controller
    setSubmitting(true)
    setPollFailed(false)
    setError(null)
    try {
      const incoming = await asrApi.start(
        { projectId, sourceId: source.id, signal: controller.signal },
        language,
      )
      if (!asrRequestMatches(requestVersion.current, responseVersion)) return
      localStorage.setItem(asrJobStorageKey(projectId, source.id), incoming.id)
      setJob(incoming)
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") return
      if (asrRequestMatches(requestVersion.current, responseVersion)) setError(errorMessage(reason))
    } finally {
      if (asrRequestMatches(requestVersion.current, responseVersion)) setSubmitting(false)
    }
  }

  async function cancel(): Promise<void> {
    if (!source || !job || !isActiveAsrJob(job)) return
    jobRequest.current?.abort()
    const controller = new AbortController()
    const responseVersion = requestVersion.current + 1
    requestVersion.current = responseVersion
    jobRequest.current = controller
    setSubmitting(true)
    setPollFailed(false)
    setError(null)
    try {
      const incoming = await asrApi.cancel({
        projectId,
        sourceId: source.id,
        jobId: job.id,
        signal: controller.signal,
      })
      if (asrRequestMatches(requestVersion.current, responseVersion)) setJob(incoming)
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") return
      if (asrRequestMatches(requestVersion.current, responseVersion)) setError(errorMessage(reason))
    } finally {
      if (asrRequestMatches(requestVersion.current, responseVersion)) setSubmitting(false)
    }
  }

  function retryPoll(): void {
    if (!job || !canRetryAsrPoll(job, pollFailed)) return
    setPollFailed(false)
    setError(null)
  }

  return {
    runtime,
    job,
    language,
    checking,
    submitting,
    pollFailed,
    error,
    setLanguage,
    refreshRuntime,
    start,
    cancel,
    retryPoll,
  }
}
