import clsx from 'clsx'
import type { CSSProperties } from 'react'
import './feedback.css'

export interface SkeletonProps {
  /** `rect` (default), `text` (one line of text), `circle`. */
  shape?: 'rect' | 'text' | 'circle'
  width?: number | string
  height?: number | string
  /** Aspect ratio instead of a height, e.g. "2 / 3" for a poster. */
  aspectRatio?: string
  /** CSS radius or token, e.g. "var(--radius-artwork)". */
  radius?: string
  /** Number of text lines (shape="text"); the last one is shorter. */
  lines?: number
  className?: string
  style?: CSSProperties
}

/** Shimmering placeholder shown while content loads. Hidden from assistive tech. */
export function Skeleton({
  shape = 'rect',
  width,
  height,
  aspectRatio,
  radius,
  lines = 1,
  className,
  style,
}: SkeletonProps) {
  if (shape === 'text' && lines > 1) {
    return (
      <span className={clsx('jr-skeleton-lines', className)} aria-hidden="true" style={style}>
        {Array.from({ length: lines }, (_, i) => (
          <span
            key={i}
            className="jr-skeleton jr-skeleton--text"
            style={{ width: i === lines - 1 ? '60%' : (width ?? '100%') }}
          />
        ))}
      </span>
    )
  }
  return (
    <span
      aria-hidden="true"
      className={clsx('jr-skeleton', `jr-skeleton--${shape}`, className)}
      style={{ width, height, aspectRatio, borderRadius: radius, ...style }}
    />
  )
}
