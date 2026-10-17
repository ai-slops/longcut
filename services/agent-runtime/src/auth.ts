import { stripVTControlCharacters } from "node:util";
import { run, type Invocation, type Process } from "./process.ts";
import { CODEX_VERSION } from "./policy.ts";
import { ServiceError, type CodexStatus } from "./types.ts";

export function deviceChallenge(text: string, deadline: string): CodexStatus["login"] {
  const clean = stripVTControlCharacters(text);
  const url = clean.match(/https:\/\/auth\.openai\.com\/codex\/device\b/)?.[0];
  const code = clean.match(/\b[A-Z0-9]{4,5}-[A-Z0-9]{4,5}\b/)?.[0];
  return url && code ? { url, code, expires_at: deadline } : null;
}

export class Auth {
  private loginProcess: Process | null = null;
  private checking: Promise<CodexStatus> | null = null;
  private stopping = false;
  private readonly children = new Set<Process>();
  private current: CodexStatus;
  private readonly executable: string;
  private readonly home: string;
  constructor(executable: string, home: string, available = true, detail: string | null = null) {
    this.executable = executable;
    this.home = home;
    this.current = { installed: available, version: available ? CODEX_VERSION : null, state: available ? "logged_out" : "error", auth_mode: "none", login: null, error: detail };
  }
  pending(): boolean { return this.loginProcess !== null; }
  cached(): CodexStatus { return this.current; }
  private ensureOpen(): void {
    if (this.stopping) throw new ServiceError("SHUTTING_DOWN", 503, "Codex account service is shutting down.");
    if (!this.current.installed) throw new ServiceError("CODEX_UNAVAILABLE", 503, "Codex safety verification failed; OpenAI-compatible generation remains available.");
  }
  private launch(args: readonly string[], options: Omit<Invocation, "cwd" | "home">): Process {
    this.ensureOpen();
    const child = run(this.executable, args, { ...options, cwd: "/tmp", home: this.home });
    this.children.add(child);
    void child.done.then(() => this.children.delete(child));
    return child;
  }
  async status(): Promise<CodexStatus> {
    if (!this.current.installed) return this.current;
    this.ensureOpen();
    if (this.loginProcess) return this.current;
    if (this.checking) return this.checking;
    this.checking = this.inspect();
    try { return await this.checking; } finally { this.checking = null; }
  }
  private async inspect(): Promise<CodexStatus> {
    const exit = await this.launch(["login", "status"], { timeoutMs: 15_000 }).done;
    const output = `${exit.stdout}\n${exit.stderr}`;
    const chatgpt = exit.code === 0 && /logged in using chatgpt/i.test(output);
    const unsupported = exit.code === 0 && !chatgpt;
    this.current = { installed: true, version: CODEX_VERSION, state: chatgpt ? "ready" : unsupported ? "error" : "logged_out", auth_mode: chatgpt ? "chatgpt" : unsupported ? "unsupported" : "none", login: null, error: unsupported ? "This integration requires ChatGPT sign-in, not API-key authentication." : null };
    return this.current;
  }
  async login(): Promise<CodexStatus> {
    this.ensureOpen();
    if (this.loginProcess) return this.current;
    if ((await this.status()).state === "ready") return this.current;
    const expires = new Date(Date.now() + 600_000).toISOString();
    let captured = "";
    this.current = { ...this.current, state: "login_pending", auth_mode: "none", login: null, error: null };
    const child = this.launch(["-c", 'forced_login_method="chatgpt"', "login", "--device-auth"], { timeoutMs: 600_000, onOutput: (_stream, text) => { captured += text; const login = deviceChallenge(captured, expires); if (login) this.current = { ...this.current, login }; } });
    this.loginProcess = child;
    void child.done.then(async (exit) => {
      if (this.loginProcess !== child) return;
      this.loginProcess = null;
      if (exit.code === 0 && exit.reason === "exit" && !this.stopping) await this.status();
      else this.current = { ...this.current, state: "logged_out", login: null, error: "Device sign-in did not complete. Enable device-code login in ChatGPT settings and retry." };
    });
    return this.current;
  }
  async cancel(): Promise<CodexStatus> {
    this.ensureOpen();
    const child = this.loginProcess;
    this.loginProcess = null;
    child?.cancel();
    if (child) await child.done;
    this.current = { ...this.current, state: "logged_out", login: null, error: null };
    return this.status();
  }
  async logout(): Promise<CodexStatus> {
    this.ensureOpen();
    await this.cancel();
    const exit = await this.launch(["logout"], { timeoutMs: 15_000 }).done;
    if (exit.code !== 0) throw new ServiceError("LOGOUT_FAILED", 502, "Codex could not disconnect. Retry after the current operation finishes.");
    return this.status();
  }
  async shutdown(): Promise<void> {
    this.stopping = true;
    this.loginProcess = null;
    const children = [...this.children];
    for (const child of children) child.cancel();
    await Promise.all(children.map((child) => child.done));
    await this.checking;
  }
}
