import { useEffect, useState } from "react"
import type { Command, Project, Scene, Source } from "../domain"
import { type MomentSelection, sourceHasVideo } from "../moment-workflow"
import { sceneDuration, sceneProgramStart } from "../timeline"
import { AsrControls } from "./AsrControls"
import type { EditorTab } from "./EditorTabs"
import { Inspector } from "./Inspector"
import { MediaFrame } from "./MediaFrame"
import { MomentComposer } from "./MomentComposer"
import { ProgramMonitor } from "./ProgramMonitor"
import { RangeControls } from "./RangeControls"
import { SessionImports } from "./SessionImports"
import { SourceRail } from "./SourceRail"
import { Timeline } from "./Timeline"
import { TranscriptPanel } from "./TranscriptPanel"

type SourceWorkspaceProps = {
  readonly tab: Extract<EditorTab, "Sources" | "Edit">
  readonly project: Project
  readonly busy: boolean
  readonly source: Source | null
  readonly scene: Scene | null
  readonly sourceFrame: number
  readonly programFrame: number
  readonly inFrame: number
  readonly outFrame: number
  readonly selectedSourceId: string | null
  readonly selectedSceneId: string | null
  readonly onSourceState: (source: Source) => void
  readonly onSourceFrame: (frame: number) => void
  readonly onProgramFrame: (frame: number) => void
  readonly onInFrame: (frame: number) => void
  readonly onOutFrame: (frame: number) => void
  readonly onSceneId: (id: string) => void
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onImport: (file: File, role: Source["role"]) => Promise<void>
  readonly onSessionProject: (project: Project) => void
  readonly onDemo: () => Promise<void>
}

export function SourceWorkspace(props: SourceWorkspaceProps) {
  const [transcriptRefreshKey, setTranscriptRefreshKey] = useState(0)
  const [selection, setSelection] = useState<MomentSelection | null>(null)
  const [evidenceNotice, setEvidenceNotice] = useState<string | null>(null)
  const [composing, setComposing] = useState(false)

  useEffect(() => {
    if (
      selection &&
      (selection.sourceId !== props.source?.id ||
        selection.inFrame !== props.inFrame ||
        selection.outFrame !== props.outFrame)
    ) {
      setSelection(null)
      setEvidenceNotice("Transcript evidence cleared because the selected Source range changed.")
    }
  }, [props.inFrame, props.outFrame, props.source?.id, selection])

  async function addScene(): Promise<void> {
    if (!props.source || props.outFrame <= props.inFrame) return
    await props.onCommand({
      type: "insert_scene",
      source_id: props.source.id,
      in_frame: String(props.inFrame),
      out_frame: String(props.outFrame),
      title: `Scene ${props.project.scenes.length + 1}`,
    })
  }

  function setManualRange(kind: "in" | "out", frame: number): void {
    setSelection(null)
    setEvidenceNotice("Transcript evidence cleared because the Source range was changed manually.")
    if (kind === "in") props.onInFrame(frame)
    else props.onOutFrame(frame)
  }

  return (
    <div className="edit-workspace">
      <SourceRail
        project={props.project}
        selectedId={props.selectedSourceId}
        busy={props.busy}
        onSelect={(item) => {
          setSelection(null)
          setEvidenceNotice("Transcript evidence cleared because the Source changed.")
          props.onSourceState(item)
        }}
        onImport={props.onImport}
        onDemo={props.onDemo}
      />
      <div className={`edit-center ${props.tab === "Sources" ? "edit-center--sources" : ""}`}>
        {props.tab === "Sources" ? (
          <SessionImports
            key={props.project.id}
            project={props.project}
            onProject={props.onSessionProject}
          />
        ) : null}
        <div className={`monitor-grid ${props.tab === "Sources" ? "monitor-grid--source" : ""}`}>
          <div>
            <MediaFrame
              label="SOURCE"
              source={props.source}
              frame={props.sourceFrame}
              fps={props.project.frame_rate}
              maxFrame={Math.max(0, Number(props.source?.duration_frames ?? 1) - 1)}
              onFrame={props.onSourceFrame}
            />
            <RangeControls
              inFrame={props.inFrame}
              outFrame={props.outFrame}
              current={props.sourceFrame}
              onIn={(frame) => setManualRange("in", frame)}
              onOut={(frame) => setManualRange("out", frame)}
              onAdd={() => void addScene()}
              onSaveMoment={() => setComposing(true)}
              disabled={!props.source || props.busy}
              sceneDisabled={!sourceHasVideo(props.source ?? undefined)}
            />
            {evidenceNotice ? (
              <p className="moment-evidence-notice" role="status" aria-live="polite">
                {evidenceNotice}
              </p>
            ) : null}
            {composing && props.source ? (
              <MomentComposer
                source={props.source}
                inFrame={props.inFrame}
                outFrame={props.outFrame}
                selection={selection}
                busy={props.busy}
                onCommand={props.onCommand}
                onClose={() => setComposing(false)}
              />
            ) : null}
            <AsrControls
              projectId={props.project.id}
              source={props.source}
              onTranscriptReady={() => setTranscriptRefreshKey((value) => value + 1)}
            />
            <TranscriptPanel
              key={`${props.source?.id ?? "none"}:${transcriptRefreshKey}`}
              projectId={props.project.id}
              source={props.source}
              onSelectCue={(cue) => {
                if (!props.source) return
                setSelection({
                  sourceId: props.source.id,
                  inFrame: cue.inFrame,
                  outFrame: cue.outFrame,
                  evidence: cue.evidence,
                  previewText: cue.previewText,
                })
                setEvidenceNotice(null)
                props.onSourceFrame(cue.sourceFrame)
                props.onInFrame(cue.inFrame)
                props.onOutFrame(cue.outFrame)
              }}
            />
          </div>
          {props.tab === "Edit" ? (
            <ProgramMonitor
              project={props.project}
              frame={props.programFrame}
              onFrame={props.onProgramFrame}
            />
          ) : null}
        </div>
        {props.tab === "Edit" ? (
          <Timeline
            project={props.project}
            selectedId={props.selectedSceneId}
            playhead={props.programFrame}
            onPlayhead={props.onProgramFrame}
            onSelect={(item) => {
              props.onSceneId(item.id)
              props.onProgramFrame(sceneProgramStart(props.project, item.id))
            }}
            onDelete={(item) => void props.onCommand({ type: "delete_scene", scene_id: item.id })}
            onSplit={(item) => {
              const offset = props.programFrame - sceneProgramStart(props.project, item.id)
              if (offset > 0 && offset < sceneDuration(item))
                void props.onCommand({
                  type: "split_scene",
                  scene_id: item.id,
                  offset_frame: String(offset),
                })
            }}
            onLock={(item) =>
              void props.onCommand({ type: "lock_scene", scene_id: item.id, locked: !item.locked })
            }
          />
        ) : null}
      </div>
      {props.tab === "Edit" ? (
        <Inspector
          project={props.project}
          scene={props.scene}
          programFrame={props.programFrame}
          busy={props.busy}
          onCommand={props.onCommand}
          onSeek={props.onProgramFrame}
        />
      ) : null}
    </div>
  )
}
