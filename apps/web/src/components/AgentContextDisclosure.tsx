import { Eye } from "lucide-react"
import { MAX_AI_SAMPLES } from "../ai-helpers"
import type { Scene, Source } from "../domain"
import { formatFrames } from "./Ui"

type ContextDisclosure =
  | {
      readonly type: "source"
      readonly source: Source | null
      readonly startFrame: number
      readonly endFrame: number
      readonly samples: readonly number[]
    }
  | {
      readonly type: "scenes"
      readonly scenes: readonly Scene[]
      readonly cameraSources: readonly Source[]
    }

type AgentContextDisclosureProps = {
  readonly context: ContextDisclosure
  readonly fps: 30 | 60
  readonly providerName: string
}

export function AgentContextDisclosure({
  context,
  fps,
  providerName,
}: AgentContextDisclosureProps) {
  return (
    <section className="context-disclosure" aria-labelledby="context-heading">
      <header>
        <Eye size={16} />
        <div>
          <strong id="context-heading">Context sent to {providerName}</strong>
          <p>Only this selection is authorized.</p>
        </div>
      </header>
      {context.type === "source" ? (
        <dl className="agent-facts">
          <div>
            <dt>Source metadata</dt>
            <dd>
              {context.source
                ? `${context.source.name} · ${context.source.id} · ${context.source.width}×${context.source.height} · audio ${context.source.has_audio ? "present" : "absent"}`
                : "No source selected"}
            </dd>
          </div>
          <div>
            <dt>Selected timing</dt>
            <dd>
              {formatFrames(String(context.startFrame), fps)}–
              {formatFrames(String(context.endFrame), fps)}
            </dd>
          </div>
          <div>
            <dt>Sampled visuals</dt>
            <dd>
              Up to {MAX_AI_SAMPLES} timestamped JPEGs:{" "}
              {context.samples.map((frame) => formatFrames(String(frame), fps)).join(", ") ||
                "none"}
            </dd>
          </div>
        </dl>
      ) : (
        <dl className="agent-facts">
          <div>
            <dt>Scene metadata</dt>
            <dd>
              {context.scenes.length
                ? context.scenes
                    .map(
                      (scene) =>
                        `${scene.title} (${scene.id}, source ${scene.source_id}, ${formatFrames(scene.in_frame, fps)}–${formatFrames(scene.out_frame, fps)}, camera ${scene.camera?.source_id ?? "none"})`,
                    )
                    .join("; ")
                : "No scenes selected"}
            </dd>
          </div>
          <div>
            <dt>Available camera metadata</dt>
            <dd>
              {context.cameraSources.map((source) => `${source.name} (${source.id})`).join("; ") ||
                "None"}
              . Camera recordings are not sent.
            </dd>
          </div>
          <div>
            <dt>Sampled visuals</dt>
            <dd>Up to {MAX_AI_SAMPLES} timestamped JPEGs across the selected scenes.</dd>
          </div>
        </dl>
      )}
      <p className="agent-limit">
        No audio, transcript, ASR, unseen action, or semantic review is included.
      </p>
    </section>
  )
}
