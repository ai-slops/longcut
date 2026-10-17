import assert from "node:assert/strict";
import test from "node:test";
import { openAiSettings, parseOpenAiConfig } from "../src/config.ts";
import { JobRequestSchema } from "../src/types.ts";

test("provider defaults to codex and overrides remain closed", () => {
  const request = JobRequestSchema.parse({
    id: "11111111-1111-4111-8111-111111111111",
    prompt: "Fixture",
    schema: {},
    images: [],
  });
  assert.equal(request.provider, "codex");
  assert.equal(JobRequestSchema.safeParse({ ...request, endpoint: "https://attacker.test" }).success, false);
  assert.equal(JobRequestSchema.safeParse({ ...request, images: [Buffer.from("not-jpeg").toString("base64")] }).success, false);
});

test("OpenAI-compatible configuration preserves the legacy request options", () => {
  const parsed = parseOpenAiConfig(openAiSettings({
    OPENAI_API_KEY: "fixture-key",
    OPENAI_BASE_URL: "https://api.example.test/v1",
    OPENAI_MODEL: "vision-fixture",
    OPENAI_RESPONSE_FORMAT: "json_schema",
    OPENAI_MAX_OUTPUT_TOKENS: "1024",
    OPENAI_TOKEN_PARAMETER: "max_completion_tokens",
  }));
  assert.equal(parsed.kind, "configured");
  if (parsed.kind === "configured") {
    assert.equal(parsed.config.endpoint, "https://api.example.test/v1/chat/completions");
    assert.equal(parsed.config.maxOutputTokens, 1024);
  }
});

test("OPENAI_API_BASE_URL takes precedence over OPENAI_BASE_URL", () => {
  const parsed = parseOpenAiConfig(openAiSettings({
    OPENAI_API_KEY: "fixture-key",
    OPENAI_API_BASE_URL: "https://canonical.example.test/v1",
    OPENAI_BASE_URL: "https://legacy.example.test/v1",
    OPENAI_MODEL: "vision-fixture",
  }));
  assert.equal(parsed.kind, "configured");
  if (parsed.kind === "configured") {
    assert.equal(parsed.config.endpoint, "https://canonical.example.test/v1/chat/completions");
  }
});

test("credential-bearing provider URLs are disabled without leaking secrets", () => {
  const parsed = parseOpenAiConfig({
    OPENAI_API_KEY: "fixture-secret",
    OPENAI_BASE_URL: "https://user:password@example.test/v1?secret=yes",
    OPENAI_MODEL: "vision-fixture",
  });
  assert.equal(parsed.kind, "disabled");
  assert.equal(JSON.stringify(parsed).includes("fixture-secret"), false);
  assert.equal(JSON.stringify(parsed).includes("password"), false);
  assert.equal(JSON.stringify(parsed).includes("secret=yes"), false);
});
