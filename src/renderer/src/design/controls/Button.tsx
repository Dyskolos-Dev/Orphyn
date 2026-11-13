import clsx from 'clsx'
import type { ComponentPropsWithRef, ReactNode } from 'react'
import { Spinner } from '../feedback/Spinner'
import './controls.css'

export type ButtonVariant = 'filled' | 'tinted' | 'gray' | 'plain' | 'contrast'
export type ButtonSize = 'small' | 'medium' | 'large'

export interface ButtonProps extends ComponentPropsWithRef<'button'> {
  /**
   * `filled`: accent background, the one primary action. `tinted`: accent-tinted, secondary.
   * `gray`: neutral fill. `plain`: text only. `contrast`: label-coloured (white on dark), for
   * actions over artwork such as HeroBanner.
   */
  variant?: ButtonVariant
  /** 28 / 34 / 44 px high. */
  size?: ButtonSize
  /** `rounded` (8px) or `capsule`. */
  shape?: 'rounded' | 'capsule'
  /** Leading icon, e.g. `<Play />` from lucide-react. */
  icon?: ReactNode
  trailingIcon?: ReactNode
  /** Shows a spinner, sets aria-busy and disables the button. */
  loading?: boolean
  /** Red label / background for destructive actions. */
  destructive?: boolean
  fullWidth?: boolean
}

/** Text button with filled / tinted / gray / plain styles. */
export function Button({
  variant = 'filled',
  size = 'medium',
  shape = 'rounded',
  icon,
  trailingIcon,
  loading = false,
  destructive = false,
  fullWidth = false,
  disabled,
  className,
  children,
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={clsx(
        'jr-button',
        `jr-button--${variant}`,
        `jr-button--${size}`,
        shape === 'capsule' && 'jr-button--capsule',
        destructive && 'jr-button--destructive',
        fullWidth && 'jr-button--full',
        className,
      )}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? (
        <Spinner size="small" className="jr-button__icon" />
      ) : (
        icon && (
          <span className="jr-button__icon" aria-hidden="true">
            {icon}
          </span>
        )
      )}
      {children != null && <span className="jr-button__label">{children}</span>}
      {trailingIcon && (
        <span className="jr-button__icon" aria-hidden="true">
          {trailingIcon}
        </span>
      )}
    </button>
  )
}
