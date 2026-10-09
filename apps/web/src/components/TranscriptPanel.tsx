import { FileText, Search, Upload } from "lucide-react"
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react"
import { ApiError, api } from "../api"
import type { Source } from "../domain"
import {
  type CueSelection,
  cueSelection,
  type TranscriptPage,
  transcriptRequestMatches,
} from "../transcript-domain"
import { TranscriptMetadata } from "./TranscriptMetadata"
import { Badge, Button, formatFrames } from "./Ui"

type TranscriptPanelProps = {
  readonly projectId: string
  readonly source: Source | null
  readonly onSelectCue: (selection: CueSelection) => void
}

function errorMessage(reason: unknown): string {
  if (reason instanceof ApiError) return `${reason.message} [${reason.code}]`
  if (reason instanceof Error) return reason.message
  return "An unexpected transcript error occurred."
}

export function TranscriptPanel({ projectId, source, onSelectCue }: TranscriptPanelProps) {
  const sourceId = source?.id
  const fileInput = useRef<HTMLInputElement>(null)
  const request = useRef<AbortController | null>(null)
  const requestVersion = useRef(0)
  const [searchText, setSearchText] = useState("")
  const [query, setQuery] = useState("")
  const [page, setPage] = useState<TranscriptPage | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const load = useCallback(
    async (requestedQuery: string, cursor?: string): Promise<void> => {
      if (!sourceId) return
      request.current?.abort()
      const controller = new AbortController()
      const requestSourceId = sourceId
      const responseRequestVersion = requestVersion.current + 1
      requestVersion.current = responseRequestVersion
      request.current = controller
      setBusy(true)
      setError(null)
      try {
        const incoming = await api.getTranscript(projectId, requestSourceId, {
          query: requestedQuery,
          ...(cursor ? { cursor } : {}),
          signal: controller.signal,
        })
        if (!transcriptRequestMatches(requestVersion.current, responseRequestVersion)) return
        setQuery(requestedQuery)
        setPage((current) =>
          cursor && current && incoming
            ? { ...incoming, cues: [...current.cues, ...incoming.cues] }
            : incoming,
        )
      } catch (reason) {
        if (reason instanceof DOMException && reason.name === "AbortError") return
        if (transcriptRequestMatches(requestVersion.current, responseRequestVersion)) {
          setError(errorMessage(reason))
        }
      } finally {
        if (transcriptRequestMatches(requestVersion.current, responseRequestVersion)) setBusy(false)
      }
    },
    [projectId, sourceId],
  )

  useEffect(() => {
    setSearchText("")
    setQuery("")
    setPage(null)
    setError(null)
    if (sourceId) void load("")
    return () => {
      request.current?.abort()
      requestVersion.current += 1
    }
  }, [load, sourceId])

  async function importFile(file: File): Promise<void> {
    if (!source) return
    request.current?.abort()
    const controller = new AbortController()
    const responseRequestVersion = requestVersion.current + 1
    requestVersion.current = responseRequestVersion
    request.current = controller
    setBusy(true)
    setError(null)
    try {
      const incoming = await api.importTranscript(projectId, source.id, {
        file,
        signal: controller.signal,
      })
      if (!transcriptRequestMatches(requestVersion.current, responseRequestVersion)) return
      setSearchText("")
      setQuery("")
      setPage(incoming)
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") return
      if (transcriptRequestMatches(requestVersion.current, responseRequestVersion)) {
        setError(errorMessage(reason))
      }
    } finally {
      if (transcriptRequestMatches(requestVersion.current, responseRequestVersion)) setBusy(false)
    }
  }

  function search(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    void load(searchText)
  }

  return (
    <section className="transcript-panel" aria-labelledby="transcript-title">
      <header>
        <div>
          <p className="eyebrow">Selected source</p>
          <h3 id="transcript-title">Transcript</h3>
        </div>
        {page ? <Badge>{page.cue_count} cues</Badge> : null}
      </header>
      <div className="transcript-panel__tools">
        <search>
          <form onSubmit={search}>
            <label className="visually-hidden" htmlFor="transcript-search">
              Search transcript
            </label>
            <input
              id="transcript-search"
              type="search"
              value={searchText}
              placeholder="Search Korean or cue text"
              disabled={!source || busy}
              onChange={(event) => setSearchText(event.currentTarget.value)}
            />
            <Button
              type="submit"
              variant="secondary"
              disabled={!source || busy}
              aria-label="Search transcript"
            >
              <Search size={14} /> Search
            </Button>
          </form>
        </search>
        <Button
          type="button"
          variant="secondary"
          disabled={!source || busy}
          busy={busy}
          onClick={() => fileInput.current?.click()}
        >
          <Upload size={14} /> Import SRT/VTT
        </Button>
        <input
          ref={fileInput}
          className="visually-hidden"
          type="file"
          tabIndex={-1}
          aria-label="Import source transcript file"
          accept=".srt,.vtt,text/vtt,application/x-subrip"
          onChange={(event) => {
            const file = event.currentTarget.files?.item(0)
            event.currentTarget.value = ""
            if (file) void importFile(file)
          }}
        />
      </div>
      {error ? (
        <p className="transcript-panel__error" role="alert">
          {error}
        </p>
      ) : null}
      {!source ? (
        <p className="transcript-panel__empty">
          Select a Source to import or search its transcript.
        </p>
      ) : !page ? (
        <div className="transcript-panel__empty">
          <FileText size={18} />
          <span>
            {busy
              ? "Loading transcript…"
              : error
                ? "Transcript could not be loaded."
                : "No transcript available for this Source."}
          </span>
        </div>
      ) : (
        <>
          <div className="transcript-panel__meta">
            <TranscriptMetadata page={page} />
            {query ? <span>Results for “{query}”</span> : null}
          </div>
          <ol className="transcript-cues">
            {page.cues.map((cue) => (
              <li key={cue.id}>
                <button type="button" onClick={() => onSelectCue(cueSelection(cue))}>
                  <code>{formatFrames(cue.start_frame, page.frame_rate)}</code>
                  <span>{cue.text}</span>
                </button>
              </li>
            ))}
          </ol>
          {page.next_cursor ? (
            <Button
              type="button"
              variant="ghost"
              busy={busy}
              className="transcript-panel__more"
              onClick={() => void load(query, page.next_cursor ?? undefined)}
            >
              Load more cues
            </Button>
          ) : null}
        </>
      )}
    </section>
  )
}
