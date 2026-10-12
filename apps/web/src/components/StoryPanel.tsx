import type { Command, Moment, Project } from "../domain"
import { useDraft } from "../useDraft"
import { MomentsPanel } from "./MomentsPanel"
import { SceneAssignmentPanel } from "./SceneAssignmentPanel"
import { StoryOutline } from "./StoryOutline"
import { StoryVersionLink } from "./StoryVersionLink"
import { Badge, Button, Panel, TextArea } from "./Ui"

type StoryPanelProps = {
  readonly project: Project
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
  readonly onReveal: (moment: Moment) => void
}

export function StoryPanel({ project, busy, onCommand, onReveal }: StoryPanelProps) {
  const [brief, setBrief] = useDraft(project.brief, project.id)
  return (
    <div className="workspace-panel story-panel">
      <Panel title="Editorial brief" tools={<Badge>Project-wide</Badge>}>
        <TextArea
          label="What must this edit preserve?"
          value={brief}
          onChange={(event) => setBrief(event.currentTarget.value)}
          rows={4}
          placeholder="Keep the full final attempt."
        />
        <Button
          type="button"
          busy={busy}
          onClick={() => void onCommand({ type: "set_brief", brief })}
        >
          Save brief
        </Button>
        {project.story.version !== "0" ? (
          <StoryVersionLink projectId={project.id} version={project.story.version} />
        ) : null}
      </Panel>
      <StoryOutline project={project} busy={busy} onCommand={onCommand} />
      <SceneAssignmentPanel project={project} busy={busy} onCommand={onCommand} />
      <MomentsPanel project={project} busy={busy} onCommand={onCommand} onReveal={onReveal} />
    </div>
  )
}
