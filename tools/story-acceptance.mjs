/** Real API acceptance: creates isolated synthetic data and never deletes user projects. */
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";

const base = process.env.LONGCUT_URL ?? "http://127.0.0.1:3011";
async function request(path, body, expected = 200) {
  const response = await fetch(`${base}/api/v1${path}`, {
    ...(body === undefined ? {} : {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body),
    }),
    signal: AbortSignal.timeout(120_000),
  });
  assert.equal(response.status, expected, `${path}: ${await response.clone().text()}`);
  return response.json();
}
async function until(path, predicate) {
  for (let attempt = 0; attempt < 180; attempt++) {
    const value = await request(path);
    if (predicate(value)) return value;
    await delay(1000);
  }
  assert.fail(`Timed out: ${path}`);
}

let project = await request("/projects", { title: `Story acceptance ${new Date().toISOString()}`, frame_rate: 30 });
const path = `/projects/${project.id}`;
async function command(value) {
  project = await request(`${path}/commands`, {
    command_id: randomUUID(), base_revision: project.revision, command: value,
  });
}
async function story(action) {
  await command({ type: "story", action: { expected_story_version: project.story.version, ...action } });
}
async function rejected(action, status) {
  const before = project.revision;
  await request(`${path}/commands`, {
    command_id: randomUUID(), base_revision: before,
    command: { type: "story", action: { expected_story_version: project.story.version, ...action } },
  }, status);
  assert.equal((await request(path)).revision, before);
}

// Given playable synthetic media, reviewed Moments and one unclassified manual cut.
await request(`${path}/demo`, {});
project = await until(path, (value) => value.sources[0]?.status === "ready");
for (const [label, start, end, role] of [
  ["원인을 발견한 순간", "30", "90", "failure"],
  ["해결 방법을 찾은 순간", "120", "180", "insight"],
]) {
  await command({ type: "create_moment", source_id: project.sources[0].id, start_frame: start,
    end_frame: end, label, note: "Synthetic editorial evidence", narrative_role: role, evidence: null });
  let moment = project.moments.at(-1);
  await command({ type: "review_moment", moment_id: moment.id, expected_moment_version: moment.version, review_status: "reviewed" });
  moment = project.moments.at(-1);
  await command({ type: "insert_scene_from_moment", moment_id: moment.id, expected_moment_version: moment.version, title: moment.label });
}
await command({ type: "insert_scene", source_id: project.sources[0].id, in_frame: "0", out_frame: "30", title: "보존할 수동 컷" });
const originalScenes = project.scenes.map(({ id, in_frame, out_frame }) => ({ id, in_frame, out_frame }));

// When the outline is authored, linked and reordered, output order/ranges stay unchanged.
await story({ type: "create_chapter", title: "문제 발견", purpose: "문제와 원인", before_chapter_id: null });
const firstChapter = project.story.chapters[0].id;
await story({ type: "create_beat", chapter_id: firstChapter, title: "실패와 원인", purpose: "원인 설명", target_frames: "45", before_beat_id: null });
const firstBeat = project.story.chapters[0].beats[0].id;
await story({ type: "set_beat_moment_refs", beat_id: firstBeat, moment_refs: [{ moment_id: project.moments[0].id, version: project.moments[0].version }] });
const assignedStoryVersion = project.story.version;
await story({ type: "set_scene_beat", scene_id: project.scenes[0].id, beat_id: firstBeat });
assert.equal(project.story.version, assignedStoryVersion);
await story({ type: "create_chapter", title: "해결 과정", purpose: "다음 시도", before_chapter_id: null });
const secondChapter = project.story.chapters[1].id;
await story({ type: "create_beat", chapter_id: secondChapter, title: "전략 전환", purpose: "해결", target_frames: "60", before_beat_id: null });
const secondBeat = project.story.chapters[1].beats[0].id;
await story({ type: "set_beat_moment_refs", beat_id: secondBeat, moment_refs: [{ moment_id: project.moments[1].id, version: project.moments[1].version }] });
await story({ type: "set_scene_beat", scene_id: project.scenes[1].id, beat_id: secondBeat });
await story({ type: "move_chapter", chapter_id: secondChapter, before_chapter_id: firstChapter });
assert.deepEqual(project.scenes.map(({ id, in_frame, out_frame }) => ({ id, in_frame, out_frame })), originalScenes);
assert.equal(project.scenes[2].beat_id, null);
await rejected({ type: "delete_beat", beat_id: firstBeat }, 409);
await rejected({ type: "delete_chapter", chapter_id: firstChapter }, 409);
await command({ type: "lock_scene", scene_id: project.scenes[0].id, locked: true });
await rejected({ type: "set_scene_beat", scene_id: project.scenes[0].id, beat_id: secondBeat }, 423);
await command({ type: "lock_scene", scene_id: project.scenes[0].id, locked: false });

// Then frozen Story/Moment provenance survives later outline and Moment edits.
const frozenVersion = project.story.version;
const render = await request(`${path}/renders`, { base_revision: project.revision, kind: "final" }, 202);
await story({ type: "update_chapter", chapter_id: firstChapter, title: "수정된 현재 제목", purpose: "렌더 후 수정" });
await command({ type: "archive_moment", moment_id: project.moments[0].id, expected_moment_version: project.moments[0].version });
const readyProject = await until(path, (value) => value.renders.some((item) => item.id === render.id && ["ready", "error"].includes(item.status)));
const ready = readyProject.renders.find((item) => item.id === render.id);
assert.equal(ready.status, "ready", JSON.stringify(ready));
const lineage = await (await fetch(`${base}${ready.lineage_url}`)).json();
assert.equal(lineage.story_version, frozenVersion);
assert.equal(lineage.scenes[0].chapter_title, "문제 발견");
assert.equal(lineage.scenes[0].beat_id, firstBeat);
assert.equal(lineage.scenes[2].beat_id, null);
const historical = await request(`${path}/story/versions/${frozenVersion}`);
assert.equal(historical.snapshot.version, frozenVersion);
project = await request(path);
const beforeUndo = project.story;
await command({ type: "undo" });
await command({ type: "redo" });
assert.deepEqual(project.story, beforeUndo);
console.log(JSON.stringify({ passed: true, project_id: project.id, revision: project.revision,
  story_version: project.story.version, frozen_story_version: frozenVersion,
  original_scene_order: originalScenes.map((scene) => scene.id), render: ready, lineage }, null, 2));
