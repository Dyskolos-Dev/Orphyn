import clsx from 'clsx'
import type { ReactNode } from 'react'
import './feedback.css'

export interface EmptyStateProps {
  /** Large lucide icon, e.g. `<Music />`. */
  icon?: ReactNode
  title: ReactNode
  message?: ReactNode
  /** Usually one <Button>. */
  action?: ReactNode
  /** `page` fills the content area, `inline` fits in a shelf or a sheet. */
  size?: 'page' | 'inline'
  className?: string
}

/** Centered icon + title + message + action, for empty libraries, no results, errors. */
export function EmptyState({
  icon,
  title,
  message,
  action,
  size = 'page',
  className,
}: EmptyStateProps) {
  return (
    <div className={clsx('jr-empty', `jr-empty--${size}`, className)}>
      {icon && (
        <div className="jr-empty__icon" aria-hidden="true">
          {icon}
        </div>
      )}
      <h2 className="jr-empty__title">{title}</h2>
      {message && <p className="jr-empty__message">{message}</p>}
      {action && <div className="jr-empty__action">{action}</div>}
    </div>
  )
}
