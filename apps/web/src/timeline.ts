import type { Project, Scene } from "./domain"

export type ProgramLocation = {
  readonly scene: Scene
  readonly sceneStart: number
  readonly localFrame: number
  readonly sourceFrame: number
}

export function sceneDuration(scene: Scene): number {
  return Number(scene.out_frame) - Number(scene.in_frame)
}

export function programDuration(project: Project): number {
  return project.scenes.reduce((total, scene) => total + sceneDuration(scene), 0)
}

export function sceneProgramStart(project: Project, sceneId: string): number {
  let frame = 0
  for (const scene of project.scenes) {
    if (scene.id === sceneId) return frame
    frame += sceneDuration(scene)
  }
  return 0
}

export function locateProgramFrame(
  project: Project,
  requestedFrame: number,
): ProgramLocation | null {
  if (project.scenes.length === 0) return null
  const duration = programDuration(project)
  const frame = Math.max(0, Math.min(requestedFrame, Math.max(0, duration - 1)))
  let sceneStart = 0
  for (const scene of project.scenes) {
    const length = sceneDuration(scene)
    if (frame < sceneStart + length) {
      const localFrame = frame - sceneStart
      return {
        scene,
        sceneStart,
        localFrame,
        sourceFrame: Number(scene.in_frame) + localFrame,
      }
    }
    sceneStart += length
  }
  return null
}
