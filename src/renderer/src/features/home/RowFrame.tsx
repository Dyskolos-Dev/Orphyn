import type { CSSProperties, ReactNode } from 'react'
import type { RowSize } from './layout'

/** Items per row shift of the shelf (a smaller number of items = bigger cards). */
const DENSITY_SHIFT: Record<Exclude<RowSize, 'auto'>, number> = { small: 1, large: -1 }

/**
 * Applies the card size chosen for a row. `auto` leaves the global appearance in charge; the
 * others move the shelf's items-per-row by one step from whatever the appearance gives.
 */
export function RowFrame({ size, children }: { size?: RowSize; children: ReactNode }) {
  if (!size || size === 'auto') return <>{children}</>
  const style = {
    '--shelf-density': `calc(var(--shelf-density-base, 0) + var(--density-items, 0) + ${DENSITY_SHIFT[size]})`,
  } as CSSProperties
  return (
    <div className="jr-home-frame" style={style}>
      {children}
    </div>
  )
}
