import type { Project } from "../domain"
import { locateProgramFrame, programDuration, sceneDuration } from "../timeline"
import { MediaFrame } from "./MediaFrame"

type ProgramMonitorProps = {
  readonly project: Project
  readonly frame: number
  readonly onFrame: (frame: number) => void
}

export function ProgramMonitor({ project, frame, onFrame }: ProgramMonitorProps) {
  const location = locateProgramFrame(project, frame)
  const source = project.sources.find((item) => item.id === location?.scene.source_id) ?? null
  const camera = location?.scene.camera ?? null
  const cameraSource = project.sources.find((item) => item.id === camera?.source_id) ?? null
  const duration = programDuration(project)

  return (
    <MediaFrame
      label="PROGRAM"
      source={source}
      frame={frame}
      mediaFrame={location?.sourceFrame ?? 0}
      fps={project.frame_rate}
      maxFrame={Math.max(0, duration - 1)}
      mediaEndFrame={
        location && location.sceneStart + sceneDuration(location.scene) === duration
          ? Number(location.scene.out_frame)
          : null
      }
      onFrame={onFrame}
      onMediaFrame={(mediaFrame) => {
        if (location)
          onFrame(
            Math.max(
              0,
              Math.min(
                duration - 1,
                location.sceneStart + mediaFrame - Number(location.scene.in_frame),
              ),
            ),
          )
      }}
      cameraSource={cameraSource}
      cameraFrame={camera ? Number(camera.in_frame) + (location?.localFrame ?? 0) : 0}
      camera={camera}
      caption={location?.scene.caption ?? ""}
      approximate
    />
  )
}
