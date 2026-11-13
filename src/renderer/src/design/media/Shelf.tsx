import clsx from 'clsx'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import {
  Children,
  isValidElement,
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import './media.css'

/**
 * Item sizing. Each preset shows a fixed number of items per row, depending on the shelf width
 * (Apple TV / Music behaviour): poster 5-8, landscape 2-5, square 4-8, round 5-9.
 */
export type ShelfItemKind = 'poster' | 'landscape' | 'square' | 'round'

export interface ShelfProps {
  /** Section title (Title 2). */
  title?: ReactNode
  subtitle?: ReactNode
  /** Router path for "See All". */
  seeAllTo?: string
  /** Click handler for "See All" when there is no route. */
  onSeeAll?(): void
  seeAllLabel?: string
  /** Sizing preset of the items. */
  kind?: ShelfItemKind
  /** The cards. Each child becomes one list item. */
  children: ReactNode
  className?: string
  /** Accessible name when `title` is not plain text. */
  'aria-label'?: string
}

const FOCUSABLE = 'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])'

/**
 * Horizontal row with scroll-snap, hover chevrons that page through it, title and "See All".
 * Arrow Left / Right move focus between items.
 */
export function Shelf({
  title,
  subtitle,
  seeAllTo,
  onSeeAll,
  seeAllLabel,
  kind = 'poster',
  children,
  className,
  'aria-label': ariaLabel,
}: ShelfProps) {
  const { t } = useTranslation('shell')
  const scroller = useRef<HTMLUListElement>(null)
  const [edges, setEdges] = useState({ back: false, forward: false })

  const measure = useCallback(() => {
    const el = scroller.current
    if (!el) return
    const back = el.scrollLeft > 4
    const forward = el.scrollLeft + el.clientWidth < el.scrollWidth - 4
    setEdges((prev) => (prev.back === back && prev.forward === forward ? prev : { back, forward }))
  }, [])

  useEffect(() => {
    measure()
    const el = scroller.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [measure, children])

  const page = (dir: 1 | -1) => {
    const el = scroller.current
    if (!el) return
    // One page = the visible width minus the gutter, so the next item lands on the gutter.
    const gutter = parseFloat(getComputedStyle(el).scrollPaddingLeft) || 0
    el.scrollBy({ left: dir * (el.clientWidth - gutter * 2), behavior: 'smooth' })
  }

  const onKeyDown = (e: KeyboardEvent<HTMLUListElement>) => {
    if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return
    const items = Array.from(e.currentTarget.children) as HTMLElement[]
    const current = items.findIndex((li) => li.contains(document.activeElement))
    if (current < 0) return
    const next = items[current + (e.key === 'ArrowRight' ? 1 : -1)]
    const target = next?.querySelector<HTMLElement>(FOCUSABLE)
    if (target) {
      e.preventDefault()
      target.focus()
      target.scrollIntoView?.({ block: 'nearest', inline: 'nearest', behavior: 'smooth' })
    }
  }

  const label = seeAllLabel ?? t('design.seeAll')
  // Children.toArray already drops null / undefined / booleans.
  const items = Children.toArray(children)

  return (
    <section
      className={clsx('jr-shelf', `jr-shelf--${kind}`, className)}
      aria-label={ariaLabel ?? (typeof title === 'string' ? title : undefined)}
    >
      {(title || seeAllTo || onSeeAll) && (
        <header className="jr-shelf__header">
          <div className="jr-shelf__titles">
            {title && <h2 className="jr-shelf__title">{title}</h2>}
            {subtitle && <p className="jr-shelf__subtitle">{subtitle}</p>}
          </div>
          {seeAllTo ? (
            <Link className="jr-shelf__see-all" to={seeAllTo}>
              {label}
            </Link>
          ) : (
            onSeeAll && (
              <button type="button" className="jr-shelf__see-all" onClick={onSeeAll}>
                {label}
              </button>
            )
          )}
        </header>
      )}
      <div className="jr-shelf__viewport">
        <ul ref={scroller} className="jr-shelf__list" onScroll={measure} onKeyDown={onKeyDown}>
          {items.map((child, i) => (
            <li
              key={isValidElement(child) && child.key != null ? child.key : i}
              className="jr-shelf__item"
            >
              {child}
            </li>
          ))}
        </ul>
        <button
          type="button"
          tabIndex={-1}
          className="jr-shelf__chevron jr-shelf__chevron--back"
          aria-label={t('design.scrollBack')}
          hidden={!edges.back}
          onClick={() => page(-1)}
        >
          <ChevronLeft aria-hidden="true" />
        </button>
        <button
          type="button"
          tabIndex={-1}
          className="jr-shelf__chevron jr-shelf__chevron--forward"
          aria-label={t('design.scrollForward')}
          hidden={!edges.forward}
          onClick={() => page(1)}
        >
          <ChevronRight aria-hidden="true" />
        </button>
      </div>
    </section>
  )
}
