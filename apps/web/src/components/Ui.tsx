import { LoaderCircle, X } from "lucide-react"
import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  TextareaHTMLAttributes,
} from "react"
import { useId } from "react"

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  readonly variant?: "primary" | "secondary" | "ghost" | "danger"
  readonly busy?: boolean
}

export function Button({
  variant = "secondary",
  busy = false,
  className = "",
  children,
  disabled,
  ...props
}: ButtonProps) {
  return (
    <button
      className={`button button--${variant} ${className}`}
      disabled={disabled || busy}
      {...props}
    >
      {busy ? <LoaderCircle aria-hidden="true" className="spin" size={15} /> : null}
      {children}
    </button>
  )
}

type FieldProps = InputHTMLAttributes<HTMLInputElement> & {
  readonly label: string
  readonly hint?: string
}

export function Field({ label, hint, id, ...props }: FieldProps) {
  const generatedId = useId()
  const fieldId = id ?? `field-${generatedId}`
  const hintId = `${fieldId}-hint`
  const describedBy = props["aria-describedby"] ?? (hint ? hintId : undefined)
  return (
    <label className="field" htmlFor={fieldId}>
      <span>{label}</span>
      <input id={fieldId} {...props} aria-describedby={describedBy} />
      {hint ? <small id={hintId}>{hint}</small> : null}
    </label>
  )
}

type TextAreaProps = TextareaHTMLAttributes<HTMLTextAreaElement> & { readonly label: string }

export function TextArea({ label, id, ...props }: TextAreaProps) {
  const generatedId = useId()
  const fieldId = id ?? `field-${generatedId}`
  return (
    <label className="field" htmlFor={fieldId}>
      <span>{label}</span>
      <textarea id={fieldId} {...props} />
    </label>
  )
}

export function Badge({
  tone = "neutral",
  children,
}: {
  readonly tone?: "neutral" | "lime" | "violet" | "success" | "warning" | "error"
  readonly children: ReactNode
}) {
  return <span className={`badge badge--${tone}`}>{children}</span>
}

export function Panel({
  title,
  tools,
  children,
  className = "",
}: {
  readonly title: string
  readonly tools?: ReactNode
  readonly children: ReactNode
  readonly className?: string
}) {
  return (
    <section className={`panel ${className}`}>
      <header className="panel__header">
        <h2>{title}</h2>
        {tools}
      </header>
      <div className="panel__body">{children}</div>
    </section>
  )
}

export function ErrorBar({
  message,
  onDismiss,
}: {
  readonly message: string
  readonly onDismiss: () => void
}) {
  return (
    <div className="error-bar" role="alert">
      <span>{message}</span>
      <button type="button" aria-label="Dismiss error" onClick={onDismiss}>
        <X size={15} />
      </button>
    </div>
  )
}

export function formatFrames(frame: string, fps: number): string {
  const value = Number(frame)
  if (!Number.isFinite(value)) return "00:00:00:00"
  const seconds = Math.floor(value / fps)
  const ff = value % fps
  const ss = seconds % 60
  const mm = Math.floor(seconds / 60) % 60
  const hh = Math.floor(seconds / 3600)
  return [hh, mm, ss, ff].map((part) => String(part).padStart(2, "0")).join(":")
}
