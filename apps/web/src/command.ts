import type { Camera, Moment, MomentEvidenceSelection, Operation, Source } from "./domain.ts"

export type Command =
  | { readonly type: "set_brief"; readonly brief: string }
  | { readonly type: "rename_project"; readonly title: string }
  | { readonly type: "set_source_role"; readonly source_id: string; readonly role: Source["role"] }
  | {
      readonly type: "insert_scene"
      readonly source_id: string
      readonly in_frame: string
      readonly out_frame: string
      readonly title: string
    }
  | {
      readonly type: "trim_scene"
      readonly scene_id: string
      readonly in_frame: string
      readonly out_frame: string
    }
  | { readonly type: "split_scene"; readonly scene_id: string; readonly offset_frame: string }
  | {
      readonly type: "move_scene"
      readonly scene_id: string
      readonly before_scene_id: string | null
    }
  | { readonly type: "delete_scene"; readonly scene_id: string }
  | {
      readonly type: "update_scene"
      readonly scene_id: string
      readonly title: string
      readonly purpose: string
      readonly caption: string
    }
  | { readonly type: "set_camera"; readonly scene_id: string; readonly camera: Camera | null }
  | { readonly type: "lock_scene"; readonly scene_id: string; readonly locked: boolean }
  | {
      readonly type: "create_moment"
      readonly source_id: string
      readonly start_frame: string
      readonly end_frame: string
      readonly label: string
      readonly note: string
      readonly narrative_role: Moment["narrative_role"]
      readonly evidence: MomentEvidenceSelection | null
    }
  | {
      readonly type: "update_moment"
      readonly moment_id: string
      readonly expected_moment_version: string
      readonly source_id: string
      readonly start_frame: string
      readonly end_frame: string
      readonly label: string
      readonly note: string
      readonly narrative_role: Moment["narrative_role"]
      readonly evidence: MomentEvidenceSelection | null
    }
  | {
      readonly type: "review_moment"
      readonly moment_id: string
      readonly expected_moment_version: string
      readonly review_status: "reviewed" | "rejected"
    }
  | {
      readonly type: "archive_moment" | "restore_moment"
      readonly moment_id: string
      readonly expected_moment_version: string
    }
  | {
      readonly type: "insert_scene_from_moment"
      readonly moment_id: string
      readonly expected_moment_version: string
      readonly title: string
    }
  | {
      readonly type: "create_feedback"
      readonly scene_id: string
      readonly start_frame: string
      readonly end_frame: string
      readonly body: string
      readonly pin: { readonly x: number; readonly y: number } | null
      readonly render_id: string | null
    }
  | { readonly type: "resolve_feedback"; readonly feedback_id: string; readonly resolved: boolean }
  | {
      readonly type: "create_change_set"
      readonly title: string
      readonly intent: string
      readonly operations: readonly Operation[]
      readonly evidence: readonly string[]
    }
  | {
      readonly type: "apply_change_set"
      readonly change_set_id: string
      readonly operation_ids: readonly string[]
    }
  | { readonly type: "reject_change_set"; readonly change_set_id: string }
  | { readonly type: "undo" }
  | { readonly type: "redo" }
