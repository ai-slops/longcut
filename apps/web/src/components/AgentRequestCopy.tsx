import { WandSparkles } from "lucide-react"
import type { AiProvider, CodexStatus, ProviderStatus } from "../ai-domain"

function isProviderReady(
  provider: AiProvider,
  status: ProviderStatus | null,
  connection: CodexStatus | null,
): boolean {
  return provider === "codex"
    ? connection?.state === "ready" && connection.auth_mode === "chatgpt"
    : status?.configured === true
}

function providerDisplayName(provider: AiProvider, status: ProviderStatus | null): string {
  return status?.name ?? (provider === "codex" ? "Codex ChatGPT" : "OpenAI-compatible API")
}

export function providerInfo(
  provider: AiProvider,
  status: ProviderStatus | null,
  connection: CodexStatus | null,
) {
  return {
    ready: isProviderReady(provider, status, connection),
    name: providerDisplayName(provider, status),
  }
}

export function AgentRequestIntro() {
  return (
    <div className="agent-intro">
      <WandSparkles size={20} />
      <div>
        <strong>Describe the edit, not the clicks</strong>
        <p>Codex can propose changes. It cannot apply them; you review each operation first.</p>
      </div>
    </div>
  )
}

export function AgentInstructionMeta({ length, maximum }: { length: number; maximum: number }) {
  return (
    <div className="instruction-meta">
      <span>Try: initial visual cut · shorten selected scenes · move camera</span>
      <code>
        {length}/{maximum}
      </code>
    </div>
  )
}

export function ProviderConsent({
  checked,
  providerName,
  onChange,
}: {
  readonly checked: boolean
  readonly providerName: string
  readonly onChange: (checked: boolean) => void
}) {
  return (
    <label className="check-field consent-field">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span>I consent to send the context listed above to {providerName} for this request.</span>
    </label>
  )
}

export function ProviderReadiness({ provider }: { readonly provider: AiProvider }) {
  return (
    <p className="muted">
      {provider === "codex"
        ? "Connect a supported ChatGPT subscription before requesting an edit."
        : "Configure the selected API provider on the server before requesting an edit."}
    </p>
  )
}
