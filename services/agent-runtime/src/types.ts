import { z } from "zod";

const id = z.string().uuid();
const frame = z.string().regex(/^\d+$/);
const camera = z.object({
  source_id: id,
  in_frame: frame,
  x: z.number(),
  y: z.number(),
  width: z.number().positive(),
  height: z.number().positive(),
  opacity: z.number().min(0).max(1),
}).strict();
const base = {
  rationale: z.string().max(2000),
  evidence_ids: z.array(z.string().max(100)).min(1).max(24),
};
export const ProposalSchema = z.object({
  title: z.string().min(1).max(200),
  summary: z.string().max(8000),
  warnings: z.array(z.string().max(2000)).max(24),
  operations: z.array(z.discriminatedUnion("type", [
    z.object({ ...base, type: z.literal("insert_scene"), source_id: id, in_frame: frame, out_frame: frame, title: z.string().max(200) }).strict(),
    z.object({ ...base, type: z.literal("trim_scene"), scene_id: id, in_frame: frame, out_frame: frame }).strict(),
    z.object({ ...base, type: z.literal("set_camera"), scene_id: id, camera: camera.nullable() }).strict(),
    z.object({ ...base, type: z.literal("update_scene"), scene_id: id, title: z.string().max(200), purpose: z.string().max(2000), caption: z.string().max(4000) }).strict(),
    z.object({ ...base, type: z.literal("delete_scene"), scene_id: id }).strict(),
  ])).max(24),
}).strict();

export const JobRequestSchema = z.object({
  id: id.transform((value) => value.toLowerCase()),
  provider: z.enum(["codex", "openai-compatible"]).default("codex"),
  prompt: z.string().min(1).max(64_000),
  schema: z.record(z.string(), z.json()),
  images: z.array(z.string().max(2_000_000).regex(/^[A-Za-z0-9+/]+={0,2}$/).refine((value) => {
    const bytes = Buffer.from(value, "base64");
    return bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff;
  }, "Only JPEG samples are accepted")).max(12),
}).strict();
export type JobRequest = z.infer<typeof JobRequestSchema>;
export type Proposal = z.infer<typeof ProposalSchema>;
export type ProviderId = JobRequest["provider"];
export type RuntimePhase = "admitted" | "provider" | "validating" | "cleaning_up" | "terminal";
export type RuntimeJob = {
  readonly id: string;
  readonly status: "running" | "succeeded" | "failed" | "cancelled";
  readonly result: Proposal | null;
  readonly error: { readonly code: string; readonly message: string } | null;
  readonly runtime: {
    readonly engine: "@slop-lab/part";
    readonly engine_version: "0.1.0";
    readonly execution_id: string;
    readonly profile_revision: string;
  };
  readonly progress: { readonly sequence: number; readonly phase: RuntimePhase };
};
export type CodexStatus = {
  readonly installed: boolean;
  readonly version: string | null;
  readonly state: "logged_out" | "login_pending" | "ready" | "busy" | "error";
  readonly auth_mode: "none" | "chatgpt" | "unsupported";
  readonly login: { readonly url: string; readonly code: string; readonly expires_at: string } | null;
  readonly error: string | null;
};
export type ProviderStatus = {
  readonly id: ProviderId;
  readonly name: string;
  readonly configured: boolean;
  readonly model: string | null;
  readonly endpoint: string | null;
  readonly detail: string | null;
};

export class ServiceError extends Error {
  readonly name = "ServiceError";
  readonly code: string;
  readonly status: number;
  constructor(code: string, status: number, message: string) {
    super(message);
    this.code = code;
    this.status = status;
  }
}
