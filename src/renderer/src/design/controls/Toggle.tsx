import clsx from 'clsx'
import { useId, type ReactNode } from 'react'
import './controls.css'

export interface ToggleProps {
  checked: boolean
  onChange(checked: boolean): void
  /** Visible label, placed before the switch (macOS settings style). */
  label?: ReactNode
  /** Secondary line under the label. */
  description?: ReactNode
  /** Accessible name when there is no visible label. */
  'aria-label'?: string
  disabled?: boolean
  size?: 'small' | 'medium'
  className?: string
  id?: string
}

/** Apple switch (role="switch"). Space / Enter toggle it. */
export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled,
  size = 'medium',
  className,
  id,
  'aria-label': ariaLabel,
}: ToggleProps) {
  const autoId = useId()
  const switchId = id ?? autoId
  const labelId = `${switchId}-label`
  const descId = `${switchId}-desc`
  const control = (
    <button
      id={switchId}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label ? undefined : ariaLabel}
      aria-labelledby={label ? labelId : undefined}
      aria-describedby={description ? descId : undefined}
      disabled={disabled}
      className={clsx('jr-toggle', `jr-toggle--${size}`, !label && className)}
      onClick={() => onChange(!checked)}
    >
      <span className="jr-toggle__knob" aria-hidden="true" />
    </button>
  )
  if (!label) return control
  return (
    <div className={clsx('jr-toggle-row', disabled && 'jr-toggle-row--disabled', className)}>
      <div className="jr-toggle-row__text">
        <label id={labelId} htmlFor={switchId} className="jr-toggle-row__label">
          {label}
        </label>
        {description && (
          <span id={descId} className="jr-toggle-row__description">
            {description}
          </span>
        )}
      </div>
      {control}
    </div>
  )
}
