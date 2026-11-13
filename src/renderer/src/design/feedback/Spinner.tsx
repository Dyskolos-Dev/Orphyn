import clsx from 'clsx'
import { useTranslation } from 'react-i18next'
import './feedback.css'

export interface SpinnerProps {
  /** 16 / 24 / 36 px. */
  size?: 'small' | 'medium' | 'large'
  /** Accessible label; defaults to "Loading". Pass `null` when a parent already announces it. */
  label?: string | null
  className?: string
}

const SPOKES = 8

/** Activity indicator (fading spokes). */
export function Spinner({ size = 'medium', label, className }: SpinnerProps) {
  const { t } = useTranslation('shell')
  const name = label === null ? undefined : (label ?? t('design.loading'))
  return (
    <span
      role={name ? 'progressbar' : undefined}
      aria-label={name}
      aria-hidden={name ? undefined : true}
      className={clsx('jr-spinner', `jr-spinner--${size}`, className)}
    >
      {Array.from({ length: SPOKES }, (_, i) => (
        <span
          key={i}
          className="jr-spinner__spoke"
          style={{
            transform: `rotate(${(360 / SPOKES) * i}deg)`,
            animationDelay: `${((i - SPOKES) / SPOKES).toFixed(3)}s`,
          }}
        />
      ))}
    </span>
  )
}
