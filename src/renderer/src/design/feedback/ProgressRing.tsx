import clsx from 'clsx'
import type { ReactNode } from 'react'
import './feedback.css'

export interface ProgressRingProps {
  /** 0..1. Omit for an indeterminate (spinning) ring. */
  value?: number
  /** Diameter in px. */
  size?: number
  strokeWidth?: number
  /** Accessible name, e.g. "Downloading". */
  label: string
  /** Content in the middle (an icon, e.g. a stop square for downloads). */
  children?: ReactNode
  className?: string
}

/** Circular progress (downloads, casting, transcoding). */
export function ProgressRing({
  value,
  size = 28,
  strokeWidth = 3,
  label,
  children,
  className,
}: ProgressRingProps) {
  const r = (size - strokeWidth) / 2
  const c = 2 * Math.PI * r
  const indeterminate = value == null
  const v = indeterminate ? 0.25 : Math.min(1, Math.max(0, value))
  return (
    <span
      role="progressbar"
      aria-label={label}
      aria-valuemin={indeterminate ? undefined : 0}
      aria-valuemax={indeterminate ? undefined : 100}
      aria-valuenow={indeterminate ? undefined : Math.round(v * 100)}
      className={clsx(
        'jr-progress-ring',
        indeterminate && 'jr-progress-ring--indeterminate',
        className,
      )}
      style={{ width: size, height: size }}
    >
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden="true">
        <circle
          className="jr-progress-ring__track"
          cx={size / 2}
          cy={size / 2}
          r={r}
          strokeWidth={strokeWidth}
          fill="none"
        />
        <circle
          className="jr-progress-ring__value"
          cx={size / 2}
          cy={size / 2}
          r={r}
          strokeWidth={strokeWidth}
          fill="none"
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c * (1 - v)}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
        />
      </svg>
      {children && <span className="jr-progress-ring__content">{children}</span>}
    </span>
  )
}
