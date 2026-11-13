import clsx from 'clsx'
import { useRef, type CSSProperties, type KeyboardEvent, type ReactNode } from 'react'
import './controls.css'

export interface SegmentedOption<T extends string> {
  value: T
  label: ReactNode
  icon?: ReactNode
  /** Accessible name when `label` is not text (icon-only segments). */
  ariaLabel?: string
  disabled?: boolean
}

export interface SegmentedControlProps<T extends string> {
  options: readonly SegmentedOption<NoInfer<T>>[]
  value: T
  onChange(value: NoInfer<T>): void
  /** Accessible name of the group. */
  'aria-label': string
  size?: 'small' | 'medium'
  /** Stretch to the container width. */
  fullWidth?: boolean
  className?: string
}

/**
 * Segmented control (radio group). Arrow keys move and select, Home / End jump; only the
 * selected segment is in the tab order.
 */
export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  size = 'medium',
  fullWidth = false,
  className,
  'aria-label': ariaLabel,
}: SegmentedControlProps<T>) {
  const refs = useRef<(HTMLButtonElement | null)[]>([])
  const index = Math.max(
    0,
    options.findIndex((o) => o.value === value),
  )

  const move = (from: number, delta: number) => {
    const n = options.length
    for (let i = 1; i <= n; i++) {
      const next = (from + delta * i + n * i) % n
      const opt = options[next]
      if (opt && !opt.disabled) {
        onChange(opt.value)
        refs.current[next]?.focus()
        return
      }
    }
  }

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') move(index, 1)
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') move(index, -1)
    else if (e.key === 'Home') move(-1, 1)
    else if (e.key === 'End') move(options.length, -1)
    else return
    e.preventDefault()
  }

  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className={clsx(
        'jr-segmented',
        `jr-segmented--${size}`,
        fullWidth && 'jr-segmented--full',
        className,
      )}
      style={{ '--jr-seg-count': options.length, '--jr-seg-index': index } as CSSProperties}
      onKeyDown={onKeyDown}
    >
      <span className="jr-segmented__thumb" aria-hidden="true" />
      {options.map((o, i) => {
        const selected = i === index
        return (
          <button
            key={o.value}
            ref={(el) => {
              refs.current[i] = el
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={o.ariaLabel}
            tabIndex={selected ? 0 : -1}
            disabled={o.disabled}
            className={clsx('jr-segmented__item', selected && 'jr-segmented__item--selected')}
            onClick={() => onChange(o.value)}
          >
            {o.icon && <span aria-hidden="true">{o.icon}</span>}
            {o.label}
          </button>
        )
      })}
    </div>
  )
}
