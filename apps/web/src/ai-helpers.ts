import type { Operation } from "./domain"

export const MAX_AI_INSTRUCTION_LENGTH = 8_000
export const MAX_AI_SCENES = 12
export const MAX_AI_SAMPLES = 12
const MAX_INTERVAL_SECONDS = 6 * 60 * 60

export function intervalLimit(durationFrames: string, fps: 30 | 60): number {
  return Math.min(Number(durationFrames), fps * MAX_INTERVAL_SECONDS)
}

export function sampledFrames(
  start: number,
  end: number,
  limit = MAX_AI_SAMPLES,
): readonly number[] {
  const count = Math.min(limit, Math.max(0, end - start))
  if (count === 0) return []
  return Array.from(
    { length: count },
    (_, index) => start + Math.floor(((end - start) * (2 * index + 1)) / (2 * count)),
  )
}

function timecode(frame: string, fps: number): string {
  const value = Number(frame)
  const seconds = Math.floor(value / fps)
  return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60, value % fps]
    .map((part) => String(part).padStart(2, "0"))
    .join(":")
}

export function describeOperation(operation: Operation, fps: number): string {
  switch (operation.type) {
    case "insert_scene":
      return `Insert “${operation.title}” from source ${operation.source_id}, ${timecode(operation.in_frame, fps)} to ${timecode(operation.out_frame, fps)}`
    case "trim_scene":
      return `Trim scene ${operation.scene_id} to ${timecode(operation.in_frame, fps)} through ${timecode(operation.out_frame, fps)}`
    case "set_camera":
      return operation.camera
        ? `Set scene ${operation.scene_id} camera to source ${operation.camera.source_id} at ${timecode(operation.camera.in_frame, fps)}`
        : `Remove camera from scene ${operation.scene_id}`
    case "update_scene":
      return `Update scene ${operation.scene_id}: “${operation.title}” · ${operation.purpose || "No purpose"} · ${operation.caption || "No caption"}`
    case "delete_scene":
      return `Delete scene ${operation.scene_id}`
  }
}

export function isActiveRun(run: { readonly status: string }): boolean {
  return run.status === "queued" || run.status === "sampling" || run.status === "generating"
}
