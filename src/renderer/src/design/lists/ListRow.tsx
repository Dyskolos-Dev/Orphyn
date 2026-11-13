import clsx from 'clsx'
import { ChevronRight } from 'lucide-react'
import type { MouseEvent, ReactNode } from 'react'
import { Link } from 'react-router'
import './lists.css'

export interface ListProps {
  /** `plain` rows with separators, or `inset` grouped (rounded card, Settings style). */
  variant?: 'plain' | 'inset'
  /** Optional heading above an inset list. */
  header?: ReactNode
  /** Optional footnote under an inset list. */
  footer?: ReactNode
  children: ReactNode
  className?: string
  'aria-label'?: string
}

/** Container for ListRow (renders a <ul>). */
export function List({
  variant = 'plain',
  header,
  footer,
  children,
  className,
  ...rest
}: ListProps) {
  return (
    <div className={clsx('jr-list-group', `jr-list-group--${variant}`, className)}>
      {header && <div className="jr-list-group__header">{header}</div>}
      <ul className="jr-list" aria-label={rest['aria-label']}>
        {children}
      </ul>
      {footer && <div className="jr-list-group__footer">{footer}</div>}
    </div>
  )
}

export interface ListRowProps {
  title: ReactNode
  subtitle?: ReactNode
  /** Icon or small Artwork on the leading side. */
  leading?: ReactNode
  /** Detail text or a control (Toggle...) on the trailing side. */
  trailing?: ReactNode
  /** Show a disclosure chevron (navigates somewhere). Defaults to true when `to` is set. */
  chevron?: boolean
  to?: string
  onClick?(): void
  onContextMenu?(e: MouseEvent): void
  selected?: boolean
  disabled?: boolean
  destructive?: boolean
  className?: string
}

/** One row: leading icon / art, title + subtitle, trailing accessory. Renders an <li>. */
export function ListRow({
  title,
  subtitle,
  leading,
  trailing,
  chevron,
  to,
  onClick,
  onContextMenu,
  selected,
  disabled,
  destructive,
  className,
}: ListRowProps) {
  const showChevron = chevron ?? Boolean(to)
  const content = (
    <>
      {leading && <span className="jr-row__leading">{leading}</span>}
      <span className="jr-row__text">
        <span className="jr-row__title">{title}</span>
        {subtitle && <span className="jr-row__subtitle">{subtitle}</span>}
      </span>
      {trailing && <span className="jr-row__trailing">{trailing}</span>}
      {showChevron && <ChevronRight className="jr-row__chevron" aria-hidden="true" />}
    </>
  )
  const cls = clsx(
    'jr-row',
    (to || onClick) && 'jr-row--interactive',
    selected && 'jr-row--selected',
    destructive && 'jr-row--destructive',
  )
  return (
    <li className={clsx('jr-list__item', className)} onContextMenu={onContextMenu}>
      {to && !disabled ? (
        <Link to={to} className={cls} aria-current={selected ? 'page' : undefined}>
          {content}
        </Link>
      ) : onClick ? (
        <button
          type="button"
          className={cls}
          onClick={onClick}
          disabled={disabled}
          aria-pressed={selected}
        >
          {content}
        </button>
      ) : (
        <div className={clsx(cls, disabled && 'jr-row--disabled')}>{content}</div>
      )}
    </li>
  )
}
