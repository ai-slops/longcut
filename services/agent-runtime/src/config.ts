import type { ProviderStatus } from "./types.ts";

export type OpenAiConfig = {
  readonly key: string;
  readonly endpoint: string;
  readonly origin: string;
  readonly model: string;
  readonly responseFormat: "json_schema" | "json_object" | "none";
  readonly maxOutputTokens: number;
  readonly tokenParameter: "max_completion_tokens" | "max_tokens";
};
type ConfigResult =
  | { readonly kind: "configured"; readonly config: OpenAiConfig; readonly status: ProviderStatus }
  | { readonly kind: "disabled"; readonly status: ProviderStatus };

const names = [
  "OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_MODEL", "OPENAI_RESPONSE_FORMAT",
  "OPENAI_MAX_OUTPUT_TOKENS", "OPENAI_TOKEN_PARAMETER",
] as const;
export type OpenAiSettings = Partial<Record<(typeof names)[number], string>>;

function disabled(model: string | null, endpoint: string | null, detail: string): ConfigResult {
  return { kind: "disabled", status: { id: "openai-compatible", name: "OpenAI-compatible API", configured: false, model, endpoint, detail } };
}

export function parseOpenAiConfig(values: OpenAiSettings): ConfigResult {
  const base = values.OPENAI_BASE_URL ?? "https://api.openai.com/v1";
  let url: URL;
  try { url = new URL(base); }
  catch { return disabled(null, null, "OPENAI_BASE_URL must be a valid HTTP(S) URL"); }
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password || url.search || url.hash || !url.hostname) {
    return disabled(null, null, "OPENAI_BASE_URL must be HTTP(S) without credentials, query, or fragment");
  }
  url.pathname = `${url.pathname.replace(/\/$/, "")}/chat/completions`;
  const endpoint = url.toString();
  const model = values.OPENAI_MODEL?.trim() || null;
  const key = values.OPENAI_API_KEY?.trim() || null;
  if (!model || !key) return disabled(model, endpoint, "Set non-empty OPENAI_API_KEY and OPENAI_MODEL server environment variables");
  if ([...model].length > 200 || /[\p{Cc}\p{Cf}]/u.test(model)) return disabled(null, endpoint, "OPENAI_MODEL must be a short single-line model identifier");
  const responseFormat = values.OPENAI_RESPONSE_FORMAT ?? "json_schema";
  if (responseFormat !== "json_schema" && responseFormat !== "json_object" && responseFormat !== "none") {
    return disabled(model, endpoint, "OPENAI_RESPONSE_FORMAT must be json_schema, json_object, or none");
  }
  const maxOutputTokens = Number(values.OPENAI_MAX_OUTPUT_TOKENS ?? "8192");
  if (!Number.isInteger(maxOutputTokens) || maxOutputTokens < 256 || maxOutputTokens > 32_768) {
    return disabled(model, endpoint, "OPENAI_MAX_OUTPUT_TOKENS must be an integer from 256 through 32768");
  }
  const tokenParameter = values.OPENAI_TOKEN_PARAMETER ?? "max_completion_tokens";
  if (tokenParameter !== "max_completion_tokens" && tokenParameter !== "max_tokens") {
    return disabled(model, endpoint, "OPENAI_TOKEN_PARAMETER must be max_completion_tokens or max_tokens");
  }
  const config = { key, endpoint, origin: url.origin, model, responseFormat, maxOutputTokens, tokenParameter } satisfies OpenAiConfig;
  return { kind: "configured", config, status: { id: "openai-compatible", name: "OpenAI-compatible API", configured: true, model, endpoint, detail: null } };
}

export function openAiSettings(environment: NodeJS.ProcessEnv): OpenAiSettings {
  const result: OpenAiSettings = {};
  for (const name of names) {
    const value = environment[name];
    if (value !== undefined) result[name] = value;
  }
  return result;
}
