import { Video } from "lucide-react"
import { type PointerEvent, useEffect, useState } from "react"
import type { Camera, Command, Project, Scene } from "../domain"
import { Button, Panel } from "./Ui"

type CameraEditorProps = {
  readonly project: Project
  readonly scene: Scene
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function CameraEditor({ project, scene, busy, onCommand }: CameraEditorProps) {
  const cameraSources = project.sources.filter((source) => source.role === "camera")
  const [camera, setCamera] = useState<Camera | null>(scene.camera)
  useEffect(() => setCamera(scene.camera), [scene])
  const source = cameraSources.find((item) => item.id === camera?.source_id)

  function choose(sourceId: string): void {
    setCamera(
      sourceId
        ? {
            source_id: sourceId,
            in_frame: "0",
            x: 0.65,
            y: 0.06,
            width: 0.3,
            height: 0.3,
            opacity: 1,
          }
        : null,
    )
  }

  function move(event: PointerEvent<HTMLDivElement>): void {
    if (!camera || scene.locked) return
    const bounds = event.currentTarget.getBoundingClientRect()
    const x = (event.clientX - bounds.left) / bounds.width - camera.width / 2
    const y = (event.clientY - bounds.top) / bounds.height - camera.height / 2
    setCamera({
      ...camera,
      x: Math.max(0, Math.min(1 - camera.width, x)),
      y: Math.max(0, Math.min(1 - camera.height, y)),
    })
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  return (
    <Panel title="Camera" tools={<Video size={14} />}>
      <label className="field">
        <span>Overlay source</span>
        <select
          value={camera?.source_id ?? ""}
          onChange={(event) => choose(event.currentTarget.value)}
          disabled={scene.locked}
        >
          <option value="">No overlay</option>
          {cameraSources.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </label>
      {camera ? (
        <>
          <div
            className="camera-canvas"
            onPointerDown={move}
            onPointerMove={(event) => {
              if (event.currentTarget.hasPointerCapture(event.pointerId)) move(event)
            }}
            onPointerUp={(event) => event.currentTarget.releasePointerCapture(event.pointerId)}
          >
            <span
              style={{
                insetInlineStart: `${camera.x * 100}%`,
                insetBlockStart: `${camera.y * 100}%`,
                inlineSize: `${camera.width * 100}%`,
                blockSize: `${camera.height * 100}%`,
                opacity: camera.opacity,
              }}
            >
              {source?.thumbnail_url ? (
                <img src={source.thumbnail_url} alt="Camera source" />
              ) : (
                "CAM"
              )}
            </span>
          </div>
          <div className="geometry-grid">
            <label>
              <span>source in frame</span>
              <input
                type="number"
                min="0"
                step="1"
                value={camera.in_frame}
                onChange={(event) => setCamera({ ...camera, in_frame: event.currentTarget.value })}
              />
            </label>
            {(["x", "y", "width", "height", "opacity"] as const).map((key) => (
              <label key={key}>
                <span>{key}</span>
                <input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  value={camera[key]}
                  onChange={(event) =>
                    setCamera({
                      ...camera,
                      [key]: Math.max(
                        0,
                        Math.min(
                          key === "x" ? 1 - camera.width : key === "y" ? 1 - camera.height : 1,
                          Number(event.currentTarget.value),
                        ),
                      ),
                    })
                  }
                />
              </label>
            ))}
          </div>
        </>
      ) : (
        <p className="muted">Import a source with the camera role before adding an overlay.</p>
      )}
      <Button
        type="button"
        busy={busy}
        disabled={scene.locked}
        onClick={() => void onCommand({ type: "set_camera", scene_id: scene.id, camera })}
      >
        Apply camera
      </Button>
    </Panel>
  )
}
