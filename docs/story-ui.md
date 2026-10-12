# Story workspace UI contract

The Story workspace is a manual chapter/beat outline over the existing edit. It is not a second timeline. `Project.scenes` array order remains the only playback and render order, and the UI states: **“Outline order does not change playback order.”**

## Workspace structure

1. **Editorial brief** keeps the existing project-wide brief workflow and offers an exact immutable Story-version preview when the current version is not `0`.
2. **Manual outline** shows at most the API-bound 64 chapters, the selected chapter's at most 128 beats, and one selected entity detail. It never renders all Moment evidence or all beat details at once.
3. **Scene playback order** renders scenes in `Project.scenes` order, 40 initially and 40 more per request. Every row shows chapter/beat badges or an explicit `Unassigned` state.
4. **Moments** retains the existing independent filter, pagination, review, archive, evidence, and Source-reveal workflows.

No chapter or beat lock affordance is shown because the API does not implement outline locks.

## Commands and concurrency

All outline and assignment mutations use the documented envelope through the existing command client:

```json
{
  "type": "story",
  "action": {
    "type": "update_beat",
    "expected_story_version": "12"
  }
}
```

The command client adds `command_id` and the current `base_revision`. Entity forms freeze the Story version and entity fields that were present when opened. If the project receives another Story version while a form is open, the UI:

- preserves local fields;
- labels the draft stale;
- disables save, delete, and cross-chapter move;
- requires **Reload current fields** before another mutation;
- never automatically rebases the draft.

Earlier/later actions use explicit `before_*_id` values. Moving a beat across chapters appends it to the selected destination; it does not reorder scenes.

## Validation and guards

- Chapter/beat title: required after trimming, at most 200 UTF-8 bytes.
- Purpose: at most 2,000 UTF-8 bytes.
- Advisory target: empty or a canonical decimal string from `1` through `2147483647`.
- Caps: 64 chapters, 512 beats total, 128 beats per chapter, and 64 Moment refs per beat.
- A chapter delete control is disabled until it has no beats and explains that beats must be deleted or moved.
- A beat delete control is disabled while any scene references it and explains that scenes must be unassigned.

Target frames are advisory only. The selected beat shows target frames, exact assigned-scene actual frames, and signed delta. Frame arithmetic uses `BigInt`; values are never converted through `Number`.

## Frozen Moment references

Only the current head of a reviewed, non-archived Moment is offered for a new link. Adding or removing a reference sends the complete replacement list in its displayed order.

Every reference is labeled `pinned vX`. Existing references remain visible after the Moment is edited or archived. A changed head is described as current head metadata, never quoted as the old version. The **Preview frozen vX** action reads the exact immutable Moment version with full project/Moment/version identity checking; quotation evidence appears only inside that explicitly opened historical preview.

## Scene assignment

Unassigned scenes are valid. Assigning or unassigning sends `set_scene_beat`; it never moves, trims, inserts, or deletes a scene. Locked scenes display their current assignment in a disabled control with the explanation: **“Unlock this scene in Edit before changing its beat.”** No hidden reorder occurs.

Scene rows show exact `out_frame - in_frame` actual frames with `BigInt`. Chapter and beat labels are derived from the current outline only; an unknown historical ID remains visibly unassigned rather than being guessed.

## Immutable Story preview

For Story versions above `0`, **Preview outline vX** requests:

`GET /api/v1/projects/{project_id}/story/versions/{version}`

The response is displayed only when `project_id`, envelope `version`, and `snapshot.version` all match the requested identity. Version `0` has no synthetic preview link.

## Responsive and accessibility behavior

- Visible copy is at least 12px.
- Controls use 44px targets at widths up to 760px and on coarse pointers.
- Chapter and beat reordering is available through deterministic keyboard-operable Earlier/Later controls.
- The chapter rail and detail become one column on narrow layouts; CJK labels use safe wrapping without clipping.
- Guard, validation, stale, lock, empty, and loading states use text in addition to color.

## Preserved state

Story mutations do not change Source selection, Source range, Source playhead, Output playhead, or selected scene state in `Editor`. Existing Moment and Source workflows remain independent. The backend owns inheritance rules for split scenes and null assignment defaults for newly inserted scenes.
