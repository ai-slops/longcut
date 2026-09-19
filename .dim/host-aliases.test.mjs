import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("host-aliases.sh", import.meta.url));
const manifest = {
  schemaVersion: 1,
  root: { path: "/fixture/project" },
  gitBaseUrl: "http://dim-gitea:3000/dim-fixture",
  hostAliases: { "dim-gitea": ["192.0.2.10"], "unneeded-service": ["192.0.2.11"] },
};

async function generate(t, input) {
  const directory = await mkdtemp(join(tmpdir(), "longcut-host-aliases-"));
  t.after(() => rm(directory, { recursive: true }));
  const path = join(directory, "manifest.json");
  await writeFile(path, JSON.stringify(input));
  return spawnSync("sh", [script], {
    encoding: "utf8",
    env: { ...process.env, DIM_PROJECT_MANIFEST: path, DIM_PROJECT_ROOT: manifest.root.path, DIM_GIT_BASE_URL: manifest.gitBaseUrl },
  });
}

test("forwards only the controller-approved Git alias into agent extra_hosts", async (t) => {
  // Given a manifest granting Git and an unrelated service address.
  // When preparing the nested agent configuration.
  const result = await generate(t, manifest);
  // Then only the required Git alias is forwarded.
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), { services: { agent: { extra_hosts: ["dim-gitea=192.0.2.10"] } } });
});

test("rejects a manifest belonging to another checkout", async (t) => {
  const result = await generate(t, { ...manifest, root: { path: "/other/project" } });
  assert.notEqual(result.status, 0);
  assert.equal(result.stdout, "");
});

test("rejects a manifest without an approved Git service address", async (t) => {
  const result = await generate(t, { ...manifest, hostAliases: {} });
  assert.notEqual(result.status, 0);
  assert.equal(result.stdout, "");
});

test("rejects a Git endpoint that does not match the lifecycle environment", async (t) => {
  const result = await generate(t, { ...manifest, gitBaseUrl: "http://untrusted.example" });
  assert.notEqual(result.status, 0);
  assert.equal(result.stdout, "");
});

test("merges the approved alias into the actual agent Compose configuration", async (t) => {
  const generated = await generate(t, manifest);
  assert.equal(generated.status, 0, generated.stderr);
  const directory = await mkdtemp(join(tmpdir(), "longcut-compose-aliases-"));
  t.after(() => rm(directory, { recursive: true }));
  const override = join(directory, "aliases.json");
  await writeFile(override, generated.stdout);
  const result = spawnSync("docker", ["compose", "--file", fileURLToPath(new URL("docker-compose.yml", import.meta.url)), "--file", override, "config", "--format", "json"], {
    encoding: "utf8",
    env: { ...process.env, DIM_GIT_USERNAME: "fixture", DIM_GIT_TOKEN: "fixture", DIM_DEVELOPMENT_GATEWAY_PORT: "31887" },
  });
  assert.equal(result.status, 0, result.stderr);
  const hosts = JSON.parse(result.stdout).services.agent.extra_hosts;
  assert.deepEqual(hosts, ["dim-gitea=192.0.2.10"]);
});
