import clsx from 'clsx'
import { Children, isValidElement, type CSSProperties, type ReactNode } from 'react'
import type { ShelfItemKind } from './Shelf'
import './media.css'

const MIN_WIDTH: Record<ShelfItemKind, number> = {
  poster: 150,
  landscape: 260,
  square: 160,
  round: 140,
}

export interface GridProps {
  /** Sizing preset (same names as Shelf) or a minimum item width in px. */
  kind?: ShelfItemKind | number
  /** Gap between items; defaults to the token for the kind. */
  gap?: string
  children: ReactNode
  className?: string
  'aria-label'?: string
}

/** Responsive library grid: as many columns as fit, every item the same width. */
export function Grid({ kind = 'poster', gap, children, className, ...rest }: GridProps) {
  const min = typeof kind === 'number' ? kind : MIN_WIDTH[kind]
  return (
    <ul
      className={clsx('jr-grid', typeof kind === 'string' && `jr-grid--${kind}`, className)}
      style={
        { '--jr-grid-min': `${min}px`, ...(gap ? { '--jr-grid-gap': gap } : {}) } as CSSProperties
      }
      aria-label={rest['aria-label']}
    >
      {Children.toArray(children).map((child, i) => (
        <li
          key={isValidElement(child) && child.key != null ? child.key : i}
          className="jr-grid__item"
        >
          {child}
        </li>
      ))}
    </ul>
  )
}
