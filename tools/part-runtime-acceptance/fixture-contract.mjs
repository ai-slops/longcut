import assert from "node:assert/strict";

export function frozenContext(prompt) {
  const match = /<untrusted_metadata>\n([^\n]+)\n<\/untrusted_metadata>/.exec(prompt);
  assert(match?.[1], "prompt must contain frozen metadata");
  return JSON.parse(match[1]);
}

export function proposalFor(prompt) {
  const context = frozenContext(prompt);
  const evidence = context.samples[0]?.evidence_id;
  assert.equal(typeof evidence, "string", "sampling must supply evidence");
  if (context.scope.type === "source") {
    const start = BigInt(context.scope.start_frame);
    const selectedEnd = BigInt(context.scope.end_frame);
    const end = start + 30n < selectedEnd ? start + 30n : selectedEnd;
    return {
      title: "PART source proposal",
      summary: "Synthetic fixture proposal for manual review.",
      warnings: [],
      operations: [{
        type: "insert_scene",
        source_id: context.scope.source_id,
        in_frame: String(start),
        out_frame: String(end),
        title: "Fixture opening",
        rationale: "Use sampled visual evidence only.",
        evidence_ids: [evidence],
      }],
    };
  }
  assert.equal(context.scope.type, "scenes");
  const selected = context.scenes[0];
  assert(selected?.scene, "scene scope must include a scene");
  return {
    title: "PART scene proposal",
    summary: "Synthetic fixture proposal for manual review.",
    warnings: [],
    operations: [{
      type: "update_scene",
      scene_id: selected.scene.id,
      title: `${selected.scene.title} reviewed`,
      purpose: selected.scene.purpose,
      caption: selected.scene.caption,
      rationale: "Use sampled visual evidence only.",
      evidence_ids: [evidence],
    }],
  };
}

export function sourceFrames(prompt) {
  const context = frozenContext(prompt);
  return context.samples.map(({ evidence_id, source_id, frame, image_index }) => ({
    evidence_id,
    source_id,
    frame,
    image_index,
  }));
}

export function inspectJpegBase64(value) {
  const bytes = Buffer.from(value, "base64");
  assert.deepEqual([...bytes.subarray(0, 3)], [0xff, 0xd8, 0xff]);
}
