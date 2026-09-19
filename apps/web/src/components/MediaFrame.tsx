import { Maximize2, Pause, Play, SkipBack } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import type { Camera, Render, Source } from "../domain"
import { Badge, formatFrames } from "./Ui"

type MediaFrameProps = {
  readonly label: "SOURCE" | "PROGRAM" | "EXACT RENDER"
  readonly source: Source | null
  readonly render?: Render | null
  readonly frame: number
  readonly mediaFrame?: number
  readonly fps: number
  readonly onFrame: (frame: number) => void
  readonly onMediaFrame?: (frame: number) => void
  readonly maxFrame?: number
  readonly cameraSource?: Source | null
  readonly cameraFrame?: number
  readonly camera?: Camera | null
  readonly caption?: string
  readonly approximate?: boolean
  readonly mediaEndFrame?: number | null
}

export function MediaFrame({
  label,
  source,
  render = null,
  frame,
  mediaFrame = frame,
  fps,
  onFrame,
  onMediaFrame = onFrame,
  maxFrame,
  cameraSource = null,
  cameraFrame = 0,
  camera = null,
  caption = "",
  approximate = false,
  mediaEndFrame = null,
}: MediaFrameProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const cameraRef = useRef<HTMLVideoElement>(null)
  const [playing, setPlaying] = useState(false)
  const playbackIntent = useRef(false)
  const [mediaError, setMediaError] = useState<string | null>(null)
  const mediaUrl = render?.media_url ?? source?.proxy_url ?? source?.media_url ?? null

  useEffect(() => {
    const video = videoRef.current
    if (
      !video ||
      video.seeking ||
      Math.abs(video.currentTime * fps - mediaFrame) < (video.paused ? 0.5 : 2)
    )
      return
    video.currentTime = mediaFrame / fps
  }, [fps, mediaFrame])

  useEffect(() => {
    const video = cameraRef.current
    if (!video || video.seeking || Math.abs(video.currentTime * fps - cameraFrame) < 2) return
    video.currentTime = cameraFrame / fps
  }, [cameraFrame, fps])

  useEffect(() => {
    if (label !== "PROGRAM") return
    const listener = (): void => toggle()
    window.addEventListener("longcut:toggle-program", listener)
    return () => window.removeEventListener("longcut:toggle-program", listener)
  })

  function toggle(): void {
    const video = videoRef.current
    if (!video) return
    playbackIntent.current = video.paused
    if (video.paused) {
      startPlayback(video)
      if (cameraRef.current) startPlayback(cameraRef.current)
    } else {
      video.pause()
      cameraRef.current?.pause()
    }
  }

  function startPlayback(video: HTMLVideoElement): void {
    void video.play().catch((error: unknown) => {
      if (error instanceof DOMException && error.name === "AbortError") return
      setMediaError(error instanceof Error ? error.message : "Media playback failed.")
    })
  }

  return (
    <section className="monitor">
      <div className="monitor__label">
        <span>{label}</span>
        {approximate ? (
          <Badge tone="warning">Approximate scene preview</Badge>
        ) : render ? (
          <Badge tone="success">Revision {render.revision} exact</Badge>
        ) : null}
      </div>
      <div className="monitor__frame">
        {mediaUrl ? (
          <video
            ref={videoRef}
            src={mediaUrl}
            width="1280"
            height="720"
            playsInline
            onLoadedMetadata={(event) => {
              setMediaError(null)
              event.currentTarget.currentTime = mediaFrame / fps
              if (playbackIntent.current) startPlayback(event.currentTarget)
            }}
            onError={() =>
              setMediaError(
                "This browser could not decode the media. Use Chrome with H.264 support.",
              )
            }
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
            onTimeUpdate={(event) => {
              const video = event.currentTarget
              if (video.paused || video.seeking) return
              const current = Math.floor(video.currentTime * fps)
              if (mediaEndFrame !== null && current >= mediaEndFrame) {
                playbackIntent.current = false
                video.pause()
                cameraRef.current?.pause()
                video.currentTime = (mediaEndFrame - 1) / fps
                onMediaFrame(mediaEndFrame - 1)
              } else onMediaFrame(current)
            }}
          >
            <track
              kind="captions"
              src={render?.vtt_url ?? undefined}
              label="English"
              srcLang="en"
            />
          </video>
        ) : (
          <div className="monitor__empty">
            <span className="aperture" />
            <strong>{label === "SOURCE" ? "Select a source" : "Assemble your first scene"}</strong>
            <p>
              {label === "SOURCE"
                ? "Source playback has its own independent playhead."
                : "Program playback follows the edit timeline."}
            </p>
          </div>
        )}
        {mediaUrl && camera && cameraSource ? (
          <video
            ref={cameraRef}
            className="camera-overlay"
            src={cameraSource.proxy_url ?? cameraSource.media_url}
            muted
            playsInline
            aria-label="Camera overlay preview"
            onLoadedMetadata={(event) => {
              event.currentTarget.currentTime = cameraFrame / fps
              if (playbackIntent.current) startPlayback(event.currentTarget)
            }}
            style={{
              insetInlineStart: `${camera.x * 100}%`,
              insetBlockStart: `${camera.y * 100}%`,
              inlineSize: `${camera.width * 100}%`,
              blockSize: `${camera.height * 100}%`,
              opacity: camera.opacity,
            }}
          />
        ) : null}
        {mediaUrl && caption ? <div className="program-caption">{caption}</div> : null}
      </div>
      {mediaError ? <p role="alert">{mediaError}</p> : null}
      <div className="transport">
        <button
          type="button"
          aria-label="Go to start"
          onClick={() => onFrame(0)}
          disabled={!mediaUrl}
        >
          <SkipBack size={15} />
        </button>
        <button
          type="button"
          aria-label={playing ? "Pause" : "Play"}
          onClick={toggle}
          disabled={!mediaUrl}
        >
          {playing ? <Pause size={16} /> : <Play size={16} />}
        </button>
        <code>{formatFrames(String(frame), fps)}</code>
        <span>{source ? source.name : render ? `${render.kind} render` : "No media"}</span>
        <button
          type="button"
          aria-label="Fullscreen"
          onClick={() => void videoRef.current?.requestFullscreen()}
          disabled={!mediaUrl}
        >
          <Maximize2 size={15} />
        </button>
      </div>
      {mediaUrl && maxFrame !== undefined ? (
        <input
          className="monitor-seek"
          type="range"
          min="0"
          max={maxFrame}
          value={Math.min(frame, maxFrame)}
          aria-label={`${label.toLowerCase()} seek`}
          onChange={(event) => onFrame(Number(event.currentTarget.value))}
        />
      ) : null}
    </section>
  )
}
