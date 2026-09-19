import { useState } from "react"
import type { Render } from "../domain"
import { Badge } from "./Ui"

export function RenderViewer({ renders }: { readonly renders: readonly Render[] }) {
  const [selectedId, setSelectedId] = useState("")
  const [compareId, setCompareId] = useState("")
  const ready = renders.filter((render) => render.status === "ready" && render.media_url)
  const selected = ready.find((render) => render.id === selectedId) ?? ready.at(-1)
  const comparison = ready.find((render) => render.id === compareId && render.id !== selected?.id)
  if (!selected) return null

  return (
    <section aria-label="Exact render review">
      <div className="geometry-grid">
        <label className="field">
          <span>Review version</span>
          <select
            value={selected.id}
            onChange={(event) => setSelectedId(event.currentTarget.value)}
          >
            {ready.map((render) => (
              <option key={render.id} value={render.id}>
                {render.kind} r{render.revision}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Compare version</span>
          <select
            value={comparison?.id ?? ""}
            onChange={(event) => setCompareId(event.currentTarget.value)}
          >
            <option value="">No comparison</option>
            {ready
              .filter((render) => render.id !== selected.id)
              .map((render) => (
                <option key={render.id} value={render.id}>
                  {render.kind} r{render.revision}
                </option>
              ))}
          </select>
        </label>
      </div>
      <div className={`monitor-grid ${comparison ? "" : "monitor-grid--source"}`}>
        {[selected, ...(comparison && comparison.id !== selected.id ? [comparison] : [])].map(
          (render) => (
            <section
              className="monitor"
              key={render.id}
              aria-label={`Exact revision ${render.revision}`}
            >
              <div className="monitor__label">
                <span>EXACT RENDER</span>
                <Badge tone="success">Revision {render.revision}</Badge>
              </div>
              <div className="monitor__frame">
                <video
                  controls
                  playsInline
                  preload="metadata"
                  src={render.media_url ?? undefined}
                  width="1920"
                  height="1080"
                >
                  <track
                    kind="captions"
                    src={render.vtt_url ?? undefined}
                    label="Project captions"
                    srcLang="en"
                    default
                  />
                </video>
              </div>
            </section>
          ),
        )}
      </div>
      {comparison ? (
        <p className="muted">
          Independent version playback. These videos retain their original render timing.
        </p>
      ) : null}
    </section>
  )
}
