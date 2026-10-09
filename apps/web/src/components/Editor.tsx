import { ArrowLeft, ChevronDown, Redo2, Undo2 } from "lucide-react"
import { useEffect, useState } from "react"
import type { Command, Project, Source } from "../domain"
import { programDuration } from "../timeline"
import { useEditorShortcuts } from "../useEditorShortcuts"
import { AgentPanel } from "./AgentPanel"
import { DeliverablesPanel } from "./DeliverablesPanel"
import { type EditorTab, EditorTabs } from "./EditorTabs"
import { ReviewPanel } from "./ReviewPanel"
import { SourceWorkspace } from "./SourceWorkspace"
import { StoryPanel } from "./StoryPanel"
import { ErrorBar } from "./Ui"

type EditorProps = {
  readonly project: Project
  readonly busy: boolean
  readonly error: string | null
  readonly onBack: () => void
  readonly onClearError: () => void
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onRefreshProject: () => Promise<void>
  readonly onImport: (file: File, role: Source["role"]) => Promise<void>
  readonly onSessionProject: (project: Project) => void
  readonly onDemo: () => Promise<void>
  readonly onRender: (kind: "preview" | "final") => Promise<void>
}

export function Editor({
  project,
  busy,
  error,
  onBack,
  onClearError,
  onCommand,
  onRefreshProject,
  onImport,
  onSessionProject,
  onDemo,
  onRender,
}: EditorProps) {
  const [tab, setTab] = useState<EditorTab>("Edit")
  const [sourceId, setSourceId] = useState<string | null>(project.sources[0]?.id ?? null)
  const [sceneId, setSceneId] = useState<string | null>(project.scenes[0]?.id ?? null)
  const [sourceFrame, setSourceFrame] = useState(0)
  const [programPosition, setProgramFrame] = useState(0)
  const duration = programDuration(project)
  const programFrame = Math.max(0, Math.min(programPosition, Math.max(0, duration - 1)))
  const [inFrame, setInFrame] = useState(0)
  const [outFrame, setOutFrame] = useState(project.frame_rate * 5)
  const source = project.sources.find((item) => item.id === sourceId) ?? null
  const scene = project.scenes.find((item) => item.id === sceneId) ?? null

  useEffect(() => {
    if (!sourceId && project.sources[0]) setSourceId(project.sources[0].id)
    if (sceneId && !project.scenes.some((item) => item.id === sceneId))
      setSceneId(project.scenes[0]?.id ?? null)
  }, [project, sceneId, sourceId])

  useEffect(() => {
    setProgramFrame((value) => Math.max(0, Math.min(value, Math.max(0, duration - 1))))
  }, [duration])

  useEditorShortcuts({
    project,
    scene,
    busy,
    tab,
    sourceFrame,
    onCommand,
    setProgramFrame,
    setInFrame,
    setOutFrame,
  })

  return (
    <main className="editor-shell">
      <header className="topbar">
        <div className="topbar__project">
          <button type="button" aria-label="Back to projects" onClick={onBack}>
            <ArrowLeft size={16} />
          </button>
          <span className="brand-mark brand-mark--small" aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          <div>
            <strong>{project.title}</strong>
            <small>
              Revision {project.revision} · {project.frame_rate} fps
            </small>
          </div>
          <ChevronDown size={13} />
        </div>
        <EditorTabs active={tab} onSelect={setTab} />
        <div className="history-actions">
          <button
            type="button"
            aria-label="Undo"
            disabled={!project.can_undo || busy}
            onClick={() => void onCommand({ type: "undo" })}
          >
            <Undo2 size={15} />
          </button>
          <button
            type="button"
            aria-label="Redo"
            disabled={!project.can_redo || busy}
            onClick={() => void onCommand({ type: "redo" })}
          >
            <Redo2 size={15} />
          </button>
        </div>
      </header>
      {error ? <ErrorBar message={error} onDismiss={onClearError} /> : null}
      {tab === "Story" ? (
        <StoryPanel
          project={project}
          busy={busy}
          onCommand={onCommand}
          onReveal={(moment) => {
            setSourceId(moment.source_id)
            setSourceFrame(Number(moment.start_frame))
            setInFrame(Number(moment.start_frame))
            setOutFrame(Number(moment.end_frame))
            setTab("Sources")
          }}
        />
      ) : null}
      {tab === "Agent" ? (
        <AgentPanel
          project={project}
          onRefreshProject={onRefreshProject}
          onReview={() => setTab("Review")}
        />
      ) : null}
      {tab === "Review" ? (
        <ReviewPanel
          project={project}
          busy={busy}
          onCommand={onCommand}
          onSeek={(frame, targetSceneId) => {
            setSceneId(targetSceneId)
            setProgramFrame(frame)
            setTab("Edit")
          }}
        />
      ) : null}
      {tab === "Deliverables" ? (
        <DeliverablesPanel project={project} busy={busy} onRender={onRender} />
      ) : null}
      {tab === "Sources" || tab === "Edit" ? (
        <SourceWorkspace
          tab={tab}
          project={project}
          busy={busy}
          source={source}
          scene={scene}
          sourceFrame={sourceFrame}
          programFrame={programFrame}
          inFrame={inFrame}
          outFrame={outFrame}
          selectedSourceId={sourceId}
          selectedSceneId={sceneId}
          onSourceState={(item) => {
            setSourceId(item.id)
            setSourceFrame(0)
            setInFrame(0)
            setOutFrame(Math.min(Number(item.duration_frames), project.frame_rate * 5))
          }}
          onSourceFrame={setSourceFrame}
          onProgramFrame={setProgramFrame}
          onInFrame={setInFrame}
          onOutFrame={setOutFrame}
          onSceneId={setSceneId}
          onCommand={onCommand}
          onImport={onImport}
          onSessionProject={onSessionProject}
          onDemo={onDemo}
        />
      ) : null}
    </main>
  )
}
