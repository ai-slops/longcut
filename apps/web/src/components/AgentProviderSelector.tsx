import { ServerCog } from "lucide-react"
import type { AiProvider, ProviderStatus } from "../ai-domain"
import { Badge, Panel } from "./Ui"

type AgentProviderSelectorProps = {
  readonly provider: AiProvider
  readonly providers: readonly ProviderStatus[]
  readonly loading: boolean
  readonly onChange: (provider: AiProvider) => void
}

export function AgentProviderSelector({
  provider,
  providers,
  loading,
  onChange,
}: AgentProviderSelectorProps) {
  const apiProvider = providers.find((item) => item.id === "openai-compatible") ?? null
  const selectedStatus = providers.find((item) => item.id === provider) ?? null
  const choices: readonly AiProvider[] = ["openai-compatible", "codex"]
  return (
    <Panel
      title="AI provider"
      tools={
        <Badge tone={selectedStatus?.configured ? "success" : "neutral"}>
          {loading ? "checking" : selectedStatus?.configured ? "configured" : "unavailable"}
        </Badge>
      }
    >
      <div className="stack provider-selector">
        <fieldset>
          <legend>Send this request with</legend>
          <div className="provider-options">
            {choices.map((id) => {
              const item = providers.find((candidate) => candidate.id === id)
              return (
                <label key={id} className="provider-option">
                  <input
                    type="radio"
                    name="ai-provider"
                    value={id}
                    checked={provider === id}
                    onChange={() => onChange(id)}
                  />
                  <span>
                    <strong>
                      {item?.name ?? (id === "codex" ? "Codex ChatGPT" : "OpenAI-compatible API")}
                    </strong>
                    <small>{item?.configured ? "Configured" : "Unavailable"}</small>
                  </span>
                </label>
              )
            })}
          </div>
        </fieldset>
        {provider === "openai-compatible" ? (
          <div className="agent-notice provider-configuration">
            <header>
              <ServerCog size={16} />
              <strong>
                {apiProvider?.configured ? "Server configured" : "Server configuration needed"}
              </strong>
            </header>
            <dl className="agent-facts">
              <div>
                <dt>Model</dt>
                <dd>{apiProvider?.model ?? "Set OPENAI_MODEL"}</dd>
              </div>
              <div>
                <dt>Endpoint</dt>
                <dd>{apiProvider?.endpoint ?? "Set OPENAI_BASE_URL"}</dd>
              </div>
            </dl>
            <p>
              {apiProvider?.detail ??
                "Set OPENAI_API_KEY, OPENAI_MODEL, and OPENAI_BASE_URL in the server .env, then recreate only the core service."}
            </p>
            <p className="agent-limit">
              API usage is billed separately from a ChatGPT subscription.
            </p>
          </div>
        ) : null}
      </div>
    </Panel>
  )
}
