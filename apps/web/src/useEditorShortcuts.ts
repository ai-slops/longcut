import { type Dispatch, type SetStateAction, useEffect } from "react"
import type { Command, Project, Scene } from "./domain"
import { programDuration } from "./timeline"

type ShortcutContext = {
  readonly project: Project
  readonly scene: Scene | null
  readonly busy: boolean
  readonly tab: string
  readonly sourceFrame: number
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly setProgramFrame: Dispatch<SetStateAction<number>>
  readonly setInFrame: (frame: number) => void
  readonly setOutFrame: (frame: number) => void
}

export function useEditorShortcuts(context: ShortcutContext): void {
  useEffect(() => {
    function shortcut(event: KeyboardEvent): void {
      const target = event.target
      if (
        event.isComposing ||
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      )
        return
      const key = event.key.toLowerCase()
      if (
        (key === "delete" || key === "backspace") &&
        context.scene &&
        !context.scene.locked &&
        !context.busy &&
        context.tab === "Edit"
      ) {
        event.preventDefault()
        void context.onCommand({ type: "delete_scene", scene_id: context.scene.id })
      }
      if (event.code === "Space") {
        event.preventDefault()
        window.dispatchEvent(new Event("longcut:toggle-program"))
      }
      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault()
        const delta = event.key === "ArrowLeft" ? -1 : 1
        context.setProgramFrame((value) =>
          Math.max(0, Math.min(value + delta, Math.max(0, programDuration(context.project) - 1))),
        )
      }
      if (key === "i") context.setInFrame(context.sourceFrame)
      if (key === "o") context.setOutFrame(context.sourceFrame)
      if ((event.metaKey || event.ctrlKey) && key === "z") {
        event.preventDefault()
        void context.onCommand({ type: event.shiftKey ? "redo" : "undo" })
      }
    }
    window.addEventListener("keydown", shortcut)
    return () => window.removeEventListener("keydown", shortcut)
  }, [context])
}
