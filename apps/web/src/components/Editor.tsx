import { ArrowLeft, ChevronDown, Redo2, Undo2 } from "lucide-react"
import { useEffect, useState } from "react"
import type { Command, Project, Source } from "../domain"
import { programDuration, sceneDuration, sceneProgramStart } from "../timeline"
import { useEditorShortcuts } from "../useEditorShortcuts"
import { AgentPanel } from "./AgentPanel"
import { DeliverablesPanel } from "./DeliverablesPanel"
import { type EditorTab, EditorTabs } from "./EditorTabs"
import { Inspector } from "./Inspector"
import { MediaFrame } from "./MediaFrame"
import { ProgramMonitor } from "./ProgramMonitor"
import { RangeControls } from "./RangeControls"
import { ReviewPanel } from "./ReviewPanel"
import { SessionImports } from "./SessionImports"
import { SourceRail } from "./SourceRail"
import { StoryPanel } from "./StoryPanel"
import { Timeline } from "./Timeline"
import { TranscriptPanel } from "./TranscriptPanel"
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

  async function addScene(): Promise<void> {
    if (!source || outFrame <= inFrame) return
    await onCommand({
      type: "insert_scene",
      source_id: source.id,
      in_frame: String(inFrame),
      out_frame: String(outFrame),
      title: `Scene ${project.scenes.length + 1}`,
    })
  }

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
      {tab === "Story" ? <StoryPanel project={project} busy={busy} onCommand={onCommand} /> : null}
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
        <div className="edit-workspace">
          <SourceRail
            project={project}
            selectedId={sourceId}
            busy={busy}
            onSelect={(item) => {
              setSourceId(item.id)
              setSourceFrame(0)
              setInFrame(0)
              setOutFrame(Math.min(Number(item.duration_frames), project.frame_rate * 5))
            }}
            onImport={onImport}
            onDemo={onDemo}
          />
          <div className={`edit-center ${tab === "Sources" ? "edit-center--sources" : ""}`}>
            {tab === "Sources" ? (
              <SessionImports key={project.id} project={project} onProject={onSessionProject} />
            ) : null}
            <div className={`monitor-grid ${tab === "Sources" ? "monitor-grid--source" : ""}`}>
              <div>
                <MediaFrame
                  label="SOURCE"
                  source={source}
                  frame={sourceFrame}
                  fps={project.frame_rate}
                  maxFrame={Math.max(0, Number(source?.duration_frames ?? 1) - 1)}
                  onFrame={setSourceFrame}
                />
                <RangeControls
                  inFrame={inFrame}
                  outFrame={outFrame}
                  current={sourceFrame}
                  onIn={setInFrame}
                  onOut={setOutFrame}
                  onAdd={() => void addScene()}
                  disabled={!source || busy}
                />
                <TranscriptPanel
                  projectId={project.id}
                  source={source}
                  onSelectCue={(selection) => {
                    setSourceFrame(selection.sourceFrame)
                    setInFrame(selection.inFrame)
                    setOutFrame(selection.outFrame)
                  }}
                />
              </div>
              {tab === "Edit" ? (
                <ProgramMonitor project={project} frame={programFrame} onFrame={setProgramFrame} />
              ) : null}
            </div>
            {tab === "Edit" ? (
              <Timeline
                project={project}
                selectedId={sceneId}
                playhead={programFrame}
                onPlayhead={setProgramFrame}
                onSelect={(item) => {
                  setSceneId(item.id)
                  setProgramFrame(sceneProgramStart(project, item.id))
                }}
                onDelete={(item) => void onCommand({ type: "delete_scene", scene_id: item.id })}
                onSplit={(item) => {
                  const offset = programFrame - sceneProgramStart(project, item.id)
                  if (offset > 0 && offset < sceneDuration(item))
                    void onCommand({
                      type: "split_scene",
                      scene_id: item.id,
                      offset_frame: String(offset),
                    })
                }}
                onLock={(item) =>
                  void onCommand({ type: "lock_scene", scene_id: item.id, locked: !item.locked })
                }
              />
            ) : null}
          </div>
          {tab === "Edit" ? (
            <Inspector
              project={project}
              scene={scene}
              programFrame={programFrame}
              busy={busy}
              onCommand={onCommand}
              onSeek={setProgramFrame}
            />
          ) : null}
        </div>
      ) : null}
    </main>
  )
}
