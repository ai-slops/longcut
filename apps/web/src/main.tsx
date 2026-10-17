import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import "@fontsource/ibm-plex-mono/400.css"
import "@fontsource/ibm-plex-sans/400.css"
import "@fontsource/ibm-plex-sans/500.css"
import "@fontsource/ibm-plex-sans/600.css"
import { App } from "./App"
import "./styles/base.css"
import "./styles/layout.css"
import "./styles/components.css"
import "./styles/responsive.css"
import "./styles/agent.css"
import "./styles/runtime.css"
import "./styles/session-import.css"
import "./styles/transcript.css"
import "./styles/word-timing.css"
import "./styles/sync-diagnostics.css"
import "./styles/moments.css"
import "./styles/story.css"

if (import.meta.env.DEV && import.meta.env.VITE_DISABLE_REACT_DEVTOOLS !== "1") {
  void import("react-grab")
  void import("react-scan")
}

const root = document.getElementById("root")
if (root instanceof HTMLElement) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  )
}
