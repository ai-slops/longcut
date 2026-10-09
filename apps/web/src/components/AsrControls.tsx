import { Ban, Mic2, RefreshCw, RotateCcw, Square } from "lucide-react"
import type { AsrRuntimeStatus } from "../asr-domain"
import { AsrLanguageSchema, canResumeAsrJob, isActiveAsrJob } from "../asr-domain"
import { canSubmitAsrResume } from "../asr-submission"
import type { Source } from "../domain"
import { Badge, Button } from "./Ui"
import { useAsrControls } from "./useAsrControls"

type AsrControlsProps = {
  readonly projectId: string
  readonly source: Source | null
  readonly onTranscriptReady: () => void
}

function availabilityMessage(status: AsrRuntimeStatus): string {
  const missing = [
    ...status.missing,
    ...(!status.cli_available && !status.missing.includes("LONGCUT_WHISPER_CLI")
      ? ["Whisper CLI file"]
      : []),
    ...(!status.model_available && !status.missing.includes("LONGCUT_WHISPER_MODEL")
      ? ["Whisper model file"]
      : []),
    ...(!status.ffmpeg_available ? ["FFmpeg"] : []),
  ]
  return `Local ASR is unavailable. Missing: ${missing.join(", ") || "runtime check"}. Configure the local server runtime, then check again.`
}

export function AsrControls(props: AsrControlsProps) {
  const { source } = props
  const control = useAsrControls(props)
  const sourceReady = source?.status === "ready" && source.has_audio
  const active = control.job ? isActiveAsrJob(control.job) : false
  const resumeEligible = control.job ? canResumeAsrJob(control.job) : false
  const resumeAvailable = control.job
    ? canSubmitAsrResume(control.job, control.runtime?.available === true, control.submitting)
    : false
  const progressLabel = control.job
    ? control.job.progress.total_chunks > 0
      ? `${control.job.progress.completed_chunks} of ${control.job.progress.total_chunks} chunks`
      : "Waiting to start"
    : null

  return (
    <section className="asr-controls" aria-labelledby="asr-controls-title">
      <header>
        <div>
          <p className="eyebrow">On this device</p>
          <h3 id="asr-controls-title">Local transcription</h3>
        </div>
        <span role="status" aria-live="polite" aria-atomic="true">
          <Badge
            tone={control.runtime?.available ? "success" : control.checking ? "warning" : "error"}
          >
            {control.checking
              ? "Checking"
              : control.runtime?.available
                ? "Available"
                : "Unavailable"}
          </Badge>
        </span>
      </header>
      {control.runtime && !control.runtime.available ? (
        <p className="asr-controls__notice">
          <Ban aria-hidden="true" size={14} /> {availabilityMessage(control.runtime)}
        </p>
      ) : null}
      <div className="asr-controls__actions">
        <label className="field" htmlFor="asr-language">
          <span>Speech language</span>
          <select
            id="asr-language"
            value={control.language}
            disabled={!source || active || control.submitting}
            onChange={(event) =>
              control.setLanguage(AsrLanguageSchema.parse(event.currentTarget.value))
            }
          >
            <option value="ko">Korean</option>
            <option value="en">English</option>
            <option value="auto">Auto-detect</option>
          </select>
        </label>
        <Button
          type="button"
          variant="primary"
          busy={control.submission === "start"}
          disabled={!control.runtime?.available || !sourceReady || active || control.submitting}
          onClick={() => void control.start()}
        >
          <Mic2 aria-hidden="true" size={14} /> Start local ASR
        </Button>
        {!control.runtime?.available ? (
          <Button
            type="button"
            variant="ghost"
            busy={control.checking}
            disabled={control.submitting}
            onClick={() => void control.refreshRuntime()}
          >
            <RefreshCw aria-hidden="true" size={14} /> Check again
          </Button>
        ) : null}
      </div>
      {source && !sourceReady ? (
        <p className="asr-controls__hint">
          The selected Source must be ready and contain audio before local ASR can start.
        </p>
      ) : null}
      {control.job ? (
        <div className="asr-controls__job">
          <div role="status" aria-live="polite" aria-atomic="true">
            <Badge
              tone={
                control.job.status === "ready"
                  ? "success"
                  : active
                    ? "warning"
                    : control.job.status === "failed"
                      ? "error"
                      : "neutral"
              }
            >
              {control.job.status}
            </Badge>
            <span>{progressLabel}</span>
          </div>
          {control.job.progress.total_chunks > 0 ? (
            <progress
              aria-label="Local ASR progress"
              value={control.job.progress.completed_chunks}
              max={control.job.progress.total_chunks}
            />
          ) : null}
          {active ? (
            <div className="asr-controls__job-actions">
              {control.pollFailed ? (
                <Button
                  type="button"
                  variant="secondary"
                  disabled={control.submitting}
                  onClick={control.retryPoll}
                >
                  <RotateCcw aria-hidden="true" size={12} /> Retry status
                </Button>
              ) : null}
              <Button
                type="button"
                variant="danger"
                busy={control.submission === "cancel"}
                disabled={control.submitting}
                onClick={() => void control.cancel()}
              >
                <Square aria-hidden="true" size={12} /> Cancel
              </Button>
            </div>
          ) : null}
          {resumeEligible ? (
            <div className="asr-controls__resume">
              <p>
                Completed chunks already retained: {control.job.progress.completed_chunks}. Resume
                first revalidates the retained inputs and local model configuration, then continues
                only if they are unchanged. It never starts automatically.
              </p>
              <Button
                type="button"
                variant="secondary"
                busy={control.submission === "resume"}
                disabled={!resumeAvailable}
                onClick={() => void control.resume()}
              >
                <RotateCcw aria-hidden="true" size={12} /> Resume transcription
              </Button>
            </div>
          ) : null}
          {control.job.error ? <p role="alert">{control.job.error}</p> : null}
        </div>
      ) : null}
      {control.error ? (
        <p className="asr-controls__error" role="alert">
          {control.error}
        </p>
      ) : null}
    </section>
  )
}
