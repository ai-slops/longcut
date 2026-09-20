import { WandSparkles } from "lucide-react"
import { useMemo, useState } from "react"
import type { AiRequest, AiScope, CodexStatus } from "../ai-domain"
import {
  intervalLimit,
  MAX_AI_INSTRUCTION_LENGTH,
  MAX_AI_SCENES,
  sampledFrames,
} from "../ai-helpers"
import type { Project } from "../domain"
import { AgentContextDisclosure } from "./AgentContextDisclosure"
import { Badge, Button, formatFrames, Panel, TextArea } from "./Ui"

type AgentRequestProps = {
  readonly project: Project
  readonly connection: CodexStatus | null
  readonly submitting: boolean
  readonly onSubmit: (request: AiRequest) => Promise<boolean>
}

export function AgentRequest({ project, connection, submitting, onSubmit }: AgentRequestProps) {
  const [scopeType, setScopeType] = useState<AiScope["type"]>("source")
  const [sourceId, setSourceId] = useState(project.sources[0]?.id ?? "")
  const [startFrame, setStartFrame] = useState(0)
  const [endFrame, setEndFrame] = useState(
    Math.min(Number(project.sources[0]?.duration_frames ?? 0), project.frame_rate * 30),
  )
  const [sceneIds, setSceneIds] = useState<readonly string[]>([])
  const [instruction, setInstruction] = useState("")
  const [consent, setConsent] = useState(false)
  const source = project.sources.find((item) => item.id === sourceId) ?? null
  const selectedScenes = project.scenes.filter((scene) => sceneIds.includes(scene.id))
  const samples = useMemo(() => sampledFrames(startFrame, endFrame), [startFrame, endFrame])
  const maxSpan = source ? intervalLimit(source.duration_frames, project.frame_rate) : 0
  const sourceValid = Boolean(
    source?.status === "ready" &&
      source.width > 0 &&
      source.height > 0 &&
      endFrame > startFrame &&
      endFrame <= Number(source.duration_frames) &&
      endFrame - startFrame <= maxSpan,
  )
  const scopeValid =
    scopeType === "source" ? sourceValid : sceneIds.length > 0 && sceneIds.length <= MAX_AI_SCENES
  const canSubmit =
    connection?.state === "ready" &&
    connection.auth_mode === "chatgpt" &&
    scopeValid &&
    instruction.trim().length > 0 &&
    consent

  function chooseSource(id: string): void {
    const next = project.sources.find((item) => item.id === id)
    setSourceId(id)
    setStartFrame(0)
    setEndFrame(Math.min(Number(next?.duration_frames ?? 0), project.frame_rate * 30))
  }

  function buildRequest(): AiRequest | null {
    if (!canSubmit) return null
    const scope: AiScope =
      scopeType === "source"
        ? {
            type: "source",
            source_id: sourceId,
            start_frame: String(startFrame),
            end_frame: String(endFrame),
          }
        : { type: "scenes", scene_ids: [...sceneIds] }
    return {
      base_revision: project.revision,
      instruction: instruction.trim(),
      consent: true,
      scope,
    }
  }

  return (
    <Panel title="Request an AI edit" tools={<Badge tone="violet">Proposal only</Badge>}>
      <form
        className="stack agent-request"
        onSubmit={(event) => {
          event.preventDefault()
          const request = buildRequest()
          if (request)
            void onSubmit(request).then((success) => {
              if (success) {
                setInstruction("")
                setConsent(false)
              }
            })
        }}
      >
        <div className="agent-intro">
          <WandSparkles size={20} />
          <div>
            <strong>Describe the edit, not the clicks</strong>
            <p>Codex can propose changes. It cannot apply them; you review each operation first.</p>
          </div>
        </div>
        <fieldset>
          <legend>Scope</legend>
          <div className="segmented">
            <button
              type="button"
              className={scopeType === "source" ? "active" : ""}
              aria-pressed={scopeType === "source"}
              onClick={() => setScopeType("source")}
            >
              Source interval
            </button>
            <button
              type="button"
              className={scopeType === "scenes" ? "active" : ""}
              aria-pressed={scopeType === "scenes"}
              onClick={() => setScopeType("scenes")}
            >
              Existing scenes
            </button>
          </div>
        </fieldset>
        {scopeType === "source" ? (
          <div className="stack">
            <label className="field">
              <span>Source</span>
              <select
                value={sourceId}
                onChange={(event) => chooseSource(event.currentTarget.value)}
              >
                <option value="">Select a source</option>
                {project.sources
                  .filter((item) => item.width > 0 && item.height > 0)
                  .map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name}
                      {item.status !== "ready" ? ` (${item.status})` : ""}
                    </option>
                  ))}
              </select>
            </label>
            <div className="geometry-grid">
              <label className="field">
                <span>Start frame</span>
                <input
                  type="number"
                  min="0"
                  max={source ? Number(source.duration_frames) - 1 : 0}
                  value={startFrame}
                  onChange={(event) => setStartFrame(Number(event.currentTarget.value))}
                />
              </label>
              <label className="field">
                <span>End frame</span>
                <input
                  type="number"
                  min="1"
                  max={Number(source?.duration_frames ?? 0)}
                  value={endFrame}
                  onChange={(event) => setEndFrame(Number(event.currentTarget.value))}
                />
              </label>
            </div>
            {source && !sourceValid ? (
              <p className="error-copy">
                Choose a nonempty ready-source interval within its actual duration and the six-hour
                limit.
              </p>
            ) : null}
          </div>
        ) : (
          <div className="scene-scope">
            {project.scenes.length === 0 ? (
              <p>No scenes yet. Use a source interval to request an initial visual cut.</p>
            ) : (
              project.scenes.map((scene) => (
                <label className="check-field" key={scene.id}>
                  <input
                    type="checkbox"
                    disabled={
                      scene.locked ||
                      (!sceneIds.includes(scene.id) && sceneIds.length >= MAX_AI_SCENES)
                    }
                    checked={sceneIds.includes(scene.id)}
                    onChange={(event) =>
                      setSceneIds((current) =>
                        event.currentTarget.checked
                          ? [...current, scene.id]
                          : current.filter((id) => id !== scene.id),
                      )
                    }
                  />
                  <span>
                    <strong>{scene.title}</strong>
                    <small>
                      {scene.locked
                        ? "Locked; not available"
                        : `${formatFrames(scene.in_frame, project.frame_rate)}–${formatFrames(scene.out_frame, project.frame_rate)}`}
                    </small>
                  </span>
                </label>
              ))
            )}
          </div>
        )}
        <AgentContextDisclosure
          fps={project.frame_rate}
          context={
            scopeType === "source"
              ? { type: "source", source, startFrame, endFrame, samples }
              : {
                  type: "scenes",
                  scenes: selectedScenes,
                  cameraSources: project.sources.filter(
                    (item) =>
                      item.role === "camera" ||
                      selectedScenes.some((scene) => scene.camera?.source_id === item.id),
                  ),
                }
          }
        />
        <TextArea
          label="Instruction"
          maxLength={MAX_AI_INSTRUCTION_LENGTH}
          value={instruction}
          onChange={(event) => setInstruction(event.currentTarget.value)}
          rows={5}
          placeholder="Example: Build an initial visual cut that opens on the wide shot."
        />
        <div className="instruction-meta">
          <span>Try: initial visual cut · shorten selected scenes · move camera</span>
          <code>
            {instruction.length}/{MAX_AI_INSTRUCTION_LENGTH}
          </code>
        </div>
        <label className="check-field consent-field">
          <input
            type="checkbox"
            checked={consent}
            onChange={(event) => setConsent(event.currentTarget.checked)}
          />
          <span>I consent to send the context listed above to OpenAI for this request.</span>
        </label>
        <Button type="submit" variant="primary" busy={submitting} disabled={!canSubmit}>
          Request proposal
        </Button>
        {connection?.state !== "ready" ? (
          <p className="muted">
            Connect a supported ChatGPT subscription before requesting an edit.
          </p>
        ) : null}
      </form>
    </Panel>
  )
}
