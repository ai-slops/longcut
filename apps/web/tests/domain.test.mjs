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

test("accepts insert, update, and delete proposals with Rust null options", () => {
  // Given the three extended ChangeSet variants as serialized by Rust.
  const operations = [
    {
      id: "insert",
      scene_id: "generated-scene",
      type: "insert_scene",
      source_id: "source",
      in_frame: "120",
      out_frame: "240",
      title: "Opening",
      purpose: null,
      caption: null,
      camera: null,
      rationale: "Establish the speaker",
    },
    {
      id: "update",
      scene_id: "scene",
      type: "update_scene",
      source_id: null,
      in_frame: null,
      out_frame: null,
      title: "Clearer title",
      purpose: "Explain the workflow",
      caption: "A concise visual explanation",
      camera: null,
      rationale: "Make the scene intent explicit",
    },
    {
      id: "delete",
      scene_id: "scene-two",
      type: "delete_scene",
      source_id: null,
      in_frame: null,
      out_frame: null,
      title: null,
      purpose: null,
      caption: null,
      camera: null,
      rationale: "Remove repetition",
    },
  ]
  // When each operation crosses the wire boundary.
  const parsed = operations.map((operation) => OperationSchema.safeParse(operation))
  // Then every closed variant is accepted without dropping explicit nulls.
  assert.deepEqual(
    parsed.map((result) => result.success),
    [true, true, true],
  )
})
