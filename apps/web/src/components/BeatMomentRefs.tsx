import { useMemo, useState } from "react"
import type { Beat, Command, Project, StoryMomentRef } from "../domain"
import { createStoryCommand, linkableMomentHeads, storyLimits } from "../story-domain"
import { MomentVersionLink } from "./MomentVersionLink"
import { Badge, Button } from "./Ui"

type BeatMomentRefsProps = {
  readonly project: Project
  readonly beat: Beat
  readonly busy: boolean
  readonly onCommand: (command: Command) => Promise<boolean>
}

export function BeatMomentRefs({ project, beat, busy, onCommand }: BeatMomentRefsProps) {
  const links = useMemo(
    () => linkableMomentHeads(project.moments, beat.moment_refs),
    [beat.moment_refs, project.moments],
  )
  const [candidateId, setCandidateId] = useState(links.available[0]?.moment_id ?? "")
  const candidate = links.available.find((item) => item.moment_id === candidateId)

  function replaceRefs(refs: readonly StoryMomentRef[]): void {
    void onCommand(
      createStoryCommand(project.story.version, {
        type: "set_beat_moment_refs",
        beat_id: beat.id,
        moment_refs: refs,
      }),
    )
  }

  return (
    <section className="beat-moments">
      <header>
        <div>
          <span>Frozen Moment references</span>
          <h3>
            {beat.moment_refs.length}/{storyLimits.refsPerBeat} pinned
          </h3>
        </div>
        <Badge>Exact versions</Badge>
      </header>
      <p>
        Only current reviewed, non-archived Moment heads can be added. Existing pins remain
        historical after head changes or archival.
      </p>
      <div className="story-add-ref">
        <label className="field">
          <span>Reviewed Moment head</span>
          <select
            value={candidateId}
            disabled={busy || links.available.length === 0}
            onChange={(event) => setCandidateId(event.currentTarget.value)}
          >
            <option value="">Select a Moment</option>
            {links.available.map((ref) => {
              const moment = project.moments.find((item) => item.id === ref.moment_id)
              return (
                <option key={ref.moment_id} value={ref.moment_id}>
                  {moment?.label ?? ref.moment_id} · v{ref.version}
                </option>
              )
            })}
          </select>
        </label>
        <Button
          type="button"
          disabled={busy || !candidate || beat.moment_refs.length >= storyLimits.refsPerBeat}
          onClick={() => {
            if (candidate) replaceRefs([...beat.moment_refs, candidate])
          }}
        >
          Add pinned version
        </Button>
      </div>
      <div className="beat-moments__list">
        {beat.moment_refs.map((ref) => {
          const head = project.moments.find((item) => item.id === ref.moment_id)
          const sameHead = head?.version === ref.version
          return (
            <article key={`${ref.moment_id}:${ref.version}`}>
              <div>
                <strong>{sameHead ? head.label : ref.moment_id}</strong>
                <Badge tone="violet">pinned v{ref.version}</Badge>
                {head && !sameHead ? (
                  <small>Current head is v{head.version}; this pin remains historical.</small>
                ) : null}
                {head?.archived ? (
                  <small>Current Moment is archived; this existing pin remains valid.</small>
                ) : null}
              </div>
              <MomentVersionLink
                projectId={project.id}
                momentId={ref.moment_id}
                version={ref.version}
              />
              <Button
                type="button"
                variant="danger"
                disabled={busy}
                onClick={() =>
                  replaceRefs(beat.moment_refs.filter((item) => item.moment_id !== ref.moment_id))
                }
              >
                Remove reference
              </Button>
            </article>
          )
        })}
      </div>
    </section>
  )
}
