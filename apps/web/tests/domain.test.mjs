import assert from "node:assert/strict"
import test from "node:test"
import { OperationSchema } from "../src/domain.ts"

test("accepts camera proposals serialized with null trim fields by Rust", () => {
  // Given a real Rust Option serialization shape.
  const operation = {
    id: "operation",
    scene_id: "scene",
    type: "set_camera",
    camera: null,
    in_frame: null,
    out_frame: null,
    rationale: "Remove the camera overlay",
  }
  // When parsing the wire operation.
  const parsed = OperationSchema.safeParse(operation)
  // Then the project containing it remains readable.
  assert.equal(parsed.success, true)
})

test("rejects a trim proposal without frame boundaries", () => {
  // Given a trim operation with missing timing.
  const operation = { id: "operation", scene_id: "scene", type: "trim_scene", rationale: "Trim" }
  // When parsing the wire operation.
  const parsed = OperationSchema.safeParse(operation)
  // Then incomplete edit intent is rejected at the boundary.
  assert.equal(parsed.success, false)
})

test("accepts trim proposals with null camera fields", () => {
  // Given a valid Rust-serialized trim operation.
  const operation = {
    id: "operation",
    scene_id: "scene",
    type: "trim_scene",
    camera: null,
    in_frame: "30",
    out_frame: "60",
    rationale: "Keep the complete sentence",
  }
  // When parsing the wire operation.
  const parsed = OperationSchema.parse(operation)
  // Then exact frame strings are preserved.
  assert.equal(parsed.in_frame, "30")
  assert.equal(parsed.out_frame, "60")
})
