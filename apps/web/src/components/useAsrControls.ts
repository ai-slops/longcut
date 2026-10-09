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
  canResumeAsrJob,
  canRetryAsrPoll,
  isActiveAsrJob,
} from "../asr-domain"
import { AsrSubmissionCoordinator, type AsrSubmissionRequest } from "../asr-submission"
import type { Source } from "../domain"

type UseAsrControlsInput = {
  readonly projectId: string
  readonly source: Source | null
  readonly onTranscriptReady: () => void
}

function errorMessage(reason: unknown): string {
  if (reason instanceof ApiError) return `${reason.message} [${reason.code}]`
  if (reason instanceof Error) return reason.message
  return "An unexpected local ASR error occurred."
}

function resumeErrorMessage(reason: unknown): string {
  if (reason instanceof ApiError && reason.status === 409) {
    return `Resume was not started because its retained inputs, local model configuration, checkpoints, or current transcript changed, or the previous worker is still stopping. ${reason.message} [${reason.code}]`
  }
  return errorMessage(reason)
}

export function useAsrControls({ projectId, source, onTranscriptReady }: UseAsrControlsInput) {
  const sourceId = source?.id
  const [runtime, setRuntime] = useState<AsrRuntimeStatus | null>(null)
  const [language, setLanguage] = useState<AsrLanguage>("ko")
  const [job, setJob] = useState<AsrJob | null>(null)
  const [checking, setChecking] = useState(true)
  const [submission, setSubmission] = useState<"start" | "cancel" | "resume" | null>(null)
  const submitting = submission !== null
  const [pollFailed, setPollFailed] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [submissionCoordinator] = useState(() => new AsrSubmissionCoordinator())
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
    submissionCoordinator.invalidate()
    jobRequest.current?.abort()
    setJob(null)
    setSubmission(null)
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
          setLanguage(recovered.language)
          announcedTranscript.current = recovered.transcript_id
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
      submissionCoordinator.invalidate()
      controller.abort()
      requestVersion.current += 1
    }
  }, [projectId, sourceId, submissionCoordinator])

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

  async function submitJob(input: AsrSubmissionRequest): Promise<AsrJob | null> {
    const ticket = submissionCoordinator.begin(input.scope, input.action)
    if (!ticket) return null
    jobRequest.current?.abort()
    const controller = new AbortController()
    requestVersion.current += 1
    jobRequest.current = controller
    setSubmission(input.action)
    setPollFailed(false)
    setError(null)
    const settled = await submissionCoordinator.settle(
      ticket,
      Promise.resolve().then(() => input.operation(controller.signal)),
      input.expectedJobId,
    )
    if (settled.released) setSubmission(null)
    switch (settled.kind) {
      case "accepted":
        setJob(settled.job)
        setLanguage(settled.job.language)
        return settled.job
      case "failed":
        if (!(settled.reason instanceof DOMException && settled.reason.name === "AbortError")) {
          setError((input.formatError ?? errorMessage)(settled.reason))
        }
        return null
      case "stale":
        return null
      default: {
        const exhaustive: never = settled
        return exhaustive
      }
    }
  }

  async function start(): Promise<void> {
    if (!source || !runtime?.available) return
    const scope = { projectId, sourceId: source.id }
    const incoming = await submitJob({
      action: "start",
      scope,
      operation: (signal) => asrApi.start({ ...scope, signal }, language),
    })
    if (incoming) localStorage.setItem(asrJobStorageKey(projectId, source.id), incoming.id)
  }

  async function cancel(): Promise<void> {
    if (!source || !job || !isActiveAsrJob(job)) return
    const scope = { projectId, sourceId: source.id }
    await submitJob({
      action: "cancel",
      scope,
      expectedJobId: job.id,
      operation: (signal) => asrApi.cancel({ ...scope, jobId: job.id, signal }),
    })
  }

  async function resume(): Promise<void> {
    if (!source || !job || !runtime?.available || !canResumeAsrJob(job)) return
    const scope = { projectId, sourceId: source.id }
    await submitJob({
      action: "resume",
      scope,
      expectedJobId: job.id,
      operation: (signal) => asrApi.resume({ ...scope, jobId: job.id, signal }),
      formatError: resumeErrorMessage,
    })
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
    submission,
    pollFailed,
    error,
    setLanguage,
    refreshRuntime,
    start,
    cancel,
    resume,
    retryPoll,
  }
}
