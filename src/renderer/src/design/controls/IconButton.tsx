import clsx from 'clsx'
import type { ComponentPropsWithRef, ReactNode } from 'react'
import './controls.css'

export type IconButtonVariant = 'plain' | 'gray' | 'filled' | 'tinted' | 'glass'
export type IconButtonSize = 'small' | 'medium' | 'large'

export interface IconButtonProps extends Omit<ComponentPropsWithRef<'button'>, 'children'> {
  /** A lucide-react icon element. */
  icon: ReactNode
  /** Accessible name, also shown as the native tooltip. Required: icon buttons have no text. */
  label: string
  /**
   * `plain` (toolbar), `gray` (neutral circle), `filled` (accent circle, e.g. play),
   * `tinted`, `glass` (blurred material, over artwork).
   */
  variant?: IconButtonVariant
  /** 28 / 32 / 44 px. */
  size?: IconButtonSize
  /** Pressed state for toggles such as shuffle / repeat (sets aria-pressed). */
  pressed?: boolean
}

/** Square or circular button showing only an icon. */
export function IconButton({
  icon,
  label,
  variant = 'plain',
  size = 'medium',
  pressed,
  className,
  title,
  type = 'button',
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={title ?? label}
      aria-pressed={pressed}
      className={clsx(
        'jr-icon-button',
        `jr-icon-button--${variant}`,
        `jr-icon-button--${size}`,
        pressed && 'jr-icon-button--pressed',
        className,
      )}
      {...rest}
    >
      <span aria-hidden="true" className="jr-icon-button__icon">
        {icon}
      </span>
    </button>
  )
}
