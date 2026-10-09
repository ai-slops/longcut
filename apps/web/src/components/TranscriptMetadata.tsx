import type { TranscriptPage } from "../transcript-domain"
import { transcriptProvenance } from "../transcript-domain"

type TranscriptMetadataProps = {
  readonly page: TranscriptPage
}

export function TranscriptMetadata({ page }: TranscriptMetadataProps) {
  const provenance = transcriptProvenance(page)
  switch (provenance.kind) {
    case "subtitle_import":
      return <span>{provenance.filename}</span>
    case "local_asr":
      return (
        <div className="transcript-panel__provenance">
          <strong>Local ASR</strong>
          <span>
            Engine: {provenance.engine} · Model: {provenance.model}
          </span>
          <span>
            Language: requested {provenance.requestedLanguage}
            {provenance.resultLanguage ? ` · detected ${provenance.resultLanguage}` : ""}
          </span>
          <span>Machine-generated · not human verified</span>
        </div>
      )
    default: {
      const exhaustive: never = provenance
      return exhaustive
    }
  }
}
