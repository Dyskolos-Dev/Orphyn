/**
 * Native-feeling handling of a frameless PiP window: drag from anywhere that is not a control,
 * resize from every edge and corner, double-click to switch between a compact and a large size,
 * and a content scale that follows the window so resizing never re-lays the design out.
 */
import { useEffect, type PointerEvent as ReactPointerEvent } from 'react'
import { PhysicalSize } from '@tauri-apps/api/dpi'
import { Window } from '@tauri-apps/api/window'

/** Pixels the pointer has to travel before a press becomes a window drag (clicks stay clicks). */
export const DRAG_THRESHOLD = 4

/** Things that keep their own pointer behaviour: never start a window drag from them. */
const INTERACTIVE = 'button, a, input, select, textarea, [role="progressbar"], [data-no-drag]'

export function isInteractive(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(INTERACTIVE) !== null
}

/** Whether a press moved far enough to be a drag. */
export function pastThreshold(dx: number, dy: number, threshold = DRAG_THRESHOLD): boolean {
  return Math.hypot(dx, dy) >= threshold
}

/** Compact and large widths (logical px) the double-click switches between. */
export const COMPACT_WIDTH = 260
export const LARGE_WIDTH = 420

/** Width the double-click goes to: large from anything up to the middle, else compact. */
export function toggledWidth(currentWidth: number): number {
  return currentWidth > (COMPACT_WIDTH + LARGE_WIDTH) / 2 ? COMPACT_WIDTH : LARGE_WIDTH
}

export async function toggleCompactSize(ratio: number): Promise<void> {
  try {
    const win = Window.getCurrent()
    const scale = await win.scaleFactor()
    const size = await win.innerSize()
    const width = Math.round(toggledWidth(size.width / scale) * scale)
    await win.setSize(new PhysicalSize(width, Math.round(width / ratio)))
  } catch {
    /* not in a native window */
  }
}

/**
 * Pointer-down handler that starts the OS window drag once the pointer has moved past the
 * threshold, so a plain click on the artwork stays a click. Controls never start one.
 */
export function dragOnMove(e: ReactPointerEvent<HTMLElement>): void {
  if (e.button !== 0 || isInteractive(e.target)) return
  const startX = e.clientX
  const startY = e.clientY
  const el = e.currentTarget
  const cleanup = () => {
    el.removeEventListener('pointermove', move)
    el.removeEventListener('pointerup', cleanup)
    el.removeEventListener('pointercancel', cleanup)
  }
  const move = (m: PointerEvent) => {
    if (!pastThreshold(m.clientX - startX, m.clientY - startY)) return
    cleanup()
    void Window.getCurrent()
      .startDragging()
      .catch(() => undefined)
  }
  el.addEventListener('pointermove', move)
  el.addEventListener('pointerup', cleanup)
  el.addEventListener('pointercancel', cleanup)
}

type Direction =
  'North' | 'South' | 'East' | 'West' | 'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest'

const HANDLES: { dir: Direction; className: string }[] = [
  { dir: 'North', className: 'n' },
  { dir: 'South', className: 's' },
  { dir: 'East', className: 'e' },
  { dir: 'West', className: 'w' },
  { dir: 'NorthEast', className: 'ne' },
  { dir: 'NorthWest', className: 'nw' },
  { dir: 'SouthEast', className: 'se' },
  { dir: 'SouthWest', className: 'sw' },
]

/**
 * Invisible hit areas on every edge and corner (a frameless window has no native border), plus a
 * subtle grip in the bottom-right corner that shows on hover.
 */
export function ResizeHandles() {
  return (
    <>
      {HANDLES.map(({ dir, className }) => (
        <div
          key={dir}
          className={`pip__resize pip__resize--${className}`}
          data-no-drag
          onPointerDown={(e) => {
            if (e.button !== 0) return
            e.preventDefault()
            e.stopPropagation()
            void Window.getCurrent()
              .startResizeDragging(dir)
              .catch(() => undefined)
          }}
        />
      ))}
      <svg className="pip__grip" viewBox="0 0 12 12" aria-hidden="true">
        <path d="M11 4 4 11M11 8 8 11" />
      </svg>
    </>
  )
}

/** Width (px) the design is drawn at; the window scales it as a whole. */
export const DESIGN_WIDTH = 320

/** Publishes `--pip-scale` (window width / design width) so the card scales instead of re-laying out. */
export function usePipScale(): void {
  useEffect(() => {
    const apply = () =>
      document.documentElement.style.setProperty(
        '--pip-scale',
        String(window.innerWidth / DESIGN_WIDTH),
      )
    apply()
    window.addEventListener('resize', apply)
    return () => window.removeEventListener('resize', apply)
  }, [])
}
