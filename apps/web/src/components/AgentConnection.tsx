import { ExternalLink, Link2, LogOut, X } from "lucide-react"
import type { CodexStatus } from "../ai-domain"
import { Badge, Button, Panel } from "./Ui"

type AgentConnectionProps = {
  readonly status: CodexStatus | null
  readonly loading: boolean
  readonly onLogin: () => Promise<void>
  readonly onCancelLogin: () => Promise<void>
  readonly onLogout: () => Promise<void>
  readonly onRetry: () => Promise<void>
}

function statusTone(status: CodexStatus | null): "neutral" | "success" | "warning" | "error" {
  if (!status) return "neutral"
  if (!status.installed || status.state === "error" || status.auth_mode === "unsupported")
    return "error"
  if (status.state === "ready") return "success"
  if (status.state === "login_pending" || status.state === "busy") return "warning"
  return "neutral"
}

export function AgentConnection({
  status,
  loading,
  onLogin,
  onCancelLogin,
  onLogout,
  onRetry,
}: AgentConnectionProps) {
  const label = status?.state.replace("_", " ") ?? "checking"
  return (
    <Panel title="ChatGPT connection" tools={<Badge tone={statusTone(status)}>{label}</Badge>}>
      <div className="stack agent-connection">
        {!status ? (
          <>
            <p>Checking the local Codex service…</p>
            <Button type="button" onClick={() => void onRetry()} busy={loading}>
              Retry status
            </Button>
          </>
        ) : null}
        {status && !status.installed ? (
          <div className="agent-notice agent-notice--error">
            <strong>Codex is unavailable</strong>
            <p>
              {status.error ?? "The pinned official Codex CLI is not installed in the sidecar."}
            </p>
            <Button type="button" onClick={() => void onRetry()} busy={loading}>
              Retry
            </Button>
          </div>
        ) : null}
        {status?.installed ? (
          <dl className="agent-facts">
            <div>
              <dt>Codex CLI</dt>
              <dd>{status.version ?? "Version unavailable"}</dd>
            </div>
            <div>
              <dt>Account mode</dt>
              <dd>{status.auth_mode === "chatgpt" ? "ChatGPT subscription" : status.auth_mode}</dd>
            </div>
          </dl>
        ) : null}
        {status?.auth_mode === "unsupported" ? (
          <div className="agent-notice agent-notice--error">
            <strong>API authentication is not supported</strong>
            <p>
              Longcut only supports the official ChatGPT subscription device flow. Disconnect this
              Codex session, then connect here.
            </p>
            <Button type="button" variant="danger" onClick={() => void onLogout()} busy={loading}>
              <LogOut size={14} /> Disconnect
            </Button>
          </div>
        ) : null}
        {status?.installed &&
        status.auth_mode !== "unsupported" &&
        status.state === "logged_out" ? (
          <>
            <p>
              Connect with OpenAI’s official device flow. Longcut never asks for credentials or API
              keys.
            </p>
            <Button type="button" variant="primary" onClick={() => void onLogin()} busy={loading}>
              <Link2 size={14} /> Connect ChatGPT
            </Button>
          </>
        ) : null}
        {status?.state === "logged_out" && status.error ? (
          <p className="error-copy" role="alert">
            {status.error}
          </p>
        ) : null}
        {status?.state === "login_pending" && !status.login ? (
          <div className="stack" role="status">
            <p>Waiting for OpenAI to issue a device code…</p>
            <Button
              type="button"
              variant="ghost"
              onClick={() => void onCancelLogin()}
              busy={loading}
            >
              <X size={14} /> Cancel login
            </Button>
          </div>
        ) : null}
        {status?.state === "login_pending" && status.login ? (
          <div className="device-login">
            <p>Open the official authorization page and enter this one-time code:</p>
            <code>{status.login.code}</code>
            <a
              className="button button--primary"
              href={status.login.url}
              target="_blank"
              rel="noreferrer"
            >
              Open OpenAI <ExternalLink size={14} />
            </a>
            <small>Request expires {new Date(status.login.expires_at).toLocaleString()}.</small>
            <Button
              type="button"
              variant="ghost"
              onClick={() => void onCancelLogin()}
              busy={loading}
            >
              <X size={14} /> Cancel login
            </Button>
          </div>
        ) : null}
        {status?.auth_mode === "chatgpt" &&
        (status.state === "ready" || status.state === "busy") ? (
          <>
            <p>
              {status.state === "busy"
                ? "Codex is processing an account operation."
                : "Connected through the official ChatGPT account flow."}
            </p>
            <Button
              type="button"
              variant="ghost"
              onClick={() => void onLogout()}
              busy={loading || status.state === "busy"}
            >
              <LogOut size={14} /> Log out
            </Button>
          </>
        ) : null}
        {status?.state === "error" ? (
          <div className="agent-notice agent-notice--error">
            <strong>Connection error</strong>
            <p>{status.error ?? "Codex could not report its account state."}</p>
            <Button type="button" onClick={() => void onRetry()} busy={loading}>
              Retry status
            </Button>
          </div>
        ) : null}
      </div>
    </Panel>
  )
}
