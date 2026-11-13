import clsx from 'clsx'
import {
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react'
import './controls.css'

export interface SliderProps {
  value: number
  min?: number
  max?: number
  /** Keyboard / snapping step. Defaults to 1% of the range. */
  step?: number
  /** Called continuously while dragging and on each key press. */
  onChange(value: number): void
  /** Drag started (e.g. pause UI updates of a seek bar while scrubbing). */
  onChangeStart?(): void
  /** Drag released / key handled: commit here (e.g. actually seek). */
  onChangeEnd?(value: number): void
  /** Buffered position (seek bars), same unit as `value`. */
  buffered?: number
  /** Accessible name, e.g. "Volume" or "Seek". */
  'aria-label': string
  /** Spoken value, e.g. `(v) => formatTime(v)` for a seek bar. */
  formatValue?(value: number): string
  /**
   * `scrubber`: thin bar that thickens on hover, knob hidden until hover.
   * `regular`: always shows a round knob (settings).
   */
  variant?: 'scrubber' | 'regular'
  disabled?: boolean
  className?: string
  /** Paints the track for use over video/artwork (white on translucent). */
  onMedia?: boolean
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v))

/**
 * Horizontal slider (role="slider"). Pointer drag with capture; Arrow keys = 1 step,
 * PageUp / PageDown = 10 %, Home / End = min / max.
 */
export function Slider({
  value,
  min = 0,
  max = 1,
  step,
  onChange,
  onChangeStart,
  onChangeEnd,
  buffered,
  formatValue,
  variant = 'scrubber',
  disabled = false,
  className,
  onMedia = false,
  'aria-label': ariaLabel,
}: SliderProps) {
  const trackRef = useRef<HTMLDivElement>(null)
  const [dragging, setDragging] = useState(false)
  const range = max - min || 1
  const stepSize = step ?? range / 100
  const snap = (v: number) => {
    const snapped = step ? Math.round((v - min) / step) * step + min : v
    return clamp(Number(snapped.toFixed(10)), min, max)
  }
  const pct = (v: number) => (clamp(v, min, max) - min) / range

  const valueAt = (clientX: number) => {
    const rect = trackRef.current?.getBoundingClientRect()
    if (!rect || rect.width === 0) return value
    return snap(min + ((clientX - rect.left) / rect.width) * range)
  }

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (disabled || e.button !== 0) return
    e.currentTarget.setPointerCapture?.(e.pointerId)
    e.currentTarget.focus()
    setDragging(true)
    onChangeStart?.()
    onChange(valueAt(e.clientX))
  }
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (dragging) onChange(valueAt(e.clientX))
  }
  const endDrag = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!dragging) return
    setDragging(false)
    e.currentTarget.releasePointerCapture?.(e.pointerId)
    onChangeEnd?.(valueAt(e.clientX))
  }

  const onKeyDown = (e: KeyboardEvent) => {
    if (disabled) return
    let next: number
    switch (e.key) {
      case 'ArrowRight':
      case 'ArrowUp':
        next = value + stepSize
        break
      case 'ArrowLeft':
      case 'ArrowDown':
        next = value - stepSize
        break
      case 'PageUp':
        next = value + range / 10
        break
      case 'PageDown':
        next = value - range / 10
        break
      case 'Home':
        next = min
        break
      case 'End':
        next = max
        break
      default:
        return
    }
    e.preventDefault()
    next = snap(next)
    onChange(next)
    onChangeEnd?.(next)
  }

  return (
    <div
      ref={trackRef}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={ariaLabel}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={formatValue?.(value)}
      aria-disabled={disabled || undefined}
      aria-orientation="horizontal"
      className={clsx(
        'jr-slider',
        `jr-slider--${variant}`,
        dragging && 'jr-slider--dragging',
        disabled && 'jr-slider--disabled',
        onMedia && 'jr-slider--on-media',
        className,
      )}
      style={
        {
          '--jr-slider-value': pct(value),
          '--jr-slider-buffered': buffered == null ? 0 : pct(buffered),
        } as CSSProperties
      }
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={onKeyDown}
    >
      <span className="jr-slider__track" aria-hidden="true">
        {buffered != null && <span className="jr-slider__buffered" />}
        <span className="jr-slider__fill" />
      </span>
      <span className="jr-slider__thumb" aria-hidden="true" />
    </div>
  )
}
