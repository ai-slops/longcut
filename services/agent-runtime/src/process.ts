import { spawn } from "node:child_process";

export type Exit = {
  readonly code: number | null;
  readonly stdout: string;
  readonly stderr: string;
  readonly reason: "exit" | "cancelled" | "timeout" | "output_limit" | "policy" | "io_error";
};
export type Invocation = {
  readonly cwd: string;
  readonly home: string;
  readonly timeoutMs: number;
  readonly input?: string;
  readonly onOutput?: (stream: "stdout" | "stderr", text: string) => void;
};
export type Process = { readonly done: Promise<Exit>; readonly cancel: () => void };

export function run(executable: string, args: readonly string[], options: Invocation): Process {
  const child = spawn(executable, [...args], {
    cwd: options.cwd,
    detached: true,
    shell: false,
    env: { PATH: "/usr/local/bin:/usr/bin:/bin", HOME: "/home/longcut", CODEX_HOME: options.home, LANG: "C.UTF-8", TMPDIR: "/tmp" },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  let bytes = 0;
  let reason: Exit["reason"] = "exit";
  let stopped = false;
  let force: ReturnType<typeof setTimeout> | undefined;
  function signal(name: NodeJS.Signals): void {
    if (!child.pid) return;
    try { process.kill(-child.pid, name); }
    catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ESRCH")) child.kill(name);
    }
  }
  function stop(cause: Exit["reason"]): void {
    if (stopped) return;
    stopped = true;
    reason = cause;
    signal("SIGTERM");
    force = setTimeout(() => signal("SIGKILL"), 1000);
    force.unref();
  }
  const timer = setTimeout(() => stop("timeout"), options.timeoutMs);
  timer.unref();
  const done = new Promise<Exit>((resolve) => {
    function output(stream: "stdout" | "stderr", chunk: string): void {
      if (stopped) return;
      bytes += Buffer.byteLength(chunk);
      if (bytes > 1_048_576) { stop("output_limit"); return; }
      if (stream === "stdout") stdout += chunk; else stderr += chunk;
      try { options.onOutput?.(stream, chunk); }
      catch { stop("policy"); }
    }
    child.stdout.setEncoding("utf8").on("data", (text: string) => output("stdout", text));
    child.stderr.setEncoding("utf8").on("data", (text: string) => output("stderr", text));
    child.on("error", () => { stderr = "Codex could not be started."; });
    child.on("close", (code) => {
      clearTimeout(timer);
      if (stopped) signal("SIGKILL");
      if (force) clearTimeout(force);
      resolve({ code, stdout, stderr, reason });
    });
    child.stdin.on("error", () => stop("io_error"));
    child.stdin.end(options.input ?? "");
  });
  return { done, cancel: () => stop("cancelled") };
}
