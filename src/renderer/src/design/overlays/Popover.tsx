import clsx from 'clsx'
import { AnimatePresence, motion } from 'framer-motion'
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type AriaRole,
  type CSSProperties,
  type ReactNode,
  type RefObject,
} from 'react'
import { createPortal } from 'react-dom'
import { useFocusScope } from './focus'
import './overlays.css'

export type PopoverPlacement =
  | 'bottom-start'
  | 'bottom'
  | 'bottom-end'
  | 'top-start'
  | 'top'
  | 'top-end'
  | 'right-start'
  | 'left-start'

/** What the popover is positioned against: an element (ref) or a point (context menus). */
export type PopoverAnchor = RefObject<HTMLElement | null> | { x: number; y: number }

export interface PopoverProps {
  open: boolean
  /** Escape, click outside, or window blur. */
  onClose(): void
  anchor: PopoverAnchor
  placement?: PopoverPlacement
  /** Gap between anchor and popover, px. */
  offset?: number
  children: ReactNode
  /** `dialog` (default) or `menu` etc. */
  role?: AriaRole
  'aria-label'?: string
  /** Fixed width in px; otherwise sized by content. */
  width?: number
  className?: string
  /** Move focus inside on open (default true). */
  autoFocus?: boolean
}

const MARGIN = 8

function isPoint(a: PopoverAnchor): a is { x: number; y: number } {
  return 'x' in a && 'y' in a
}

/** Computes the popover's top-left, flipping and clamping to stay inside the viewport. */
export function computePosition(
  anchor: {
    top: number
    left: number
    right: number
    bottom: number
    width: number
    height: number
  },
  size: { width: number; height: number },
  placement: PopoverPlacement,
  offset: number,
  viewport: { width: number; height: number },
): { top: number; left: number; side: 'top' | 'bottom' | 'left' | 'right' } {
  const [side, align = 'center'] = placement.split('-') as [string, string | undefined]
  let top: number
  let left: number
  let finalSide = side as 'top' | 'bottom' | 'left' | 'right'

  if (side === 'right' || side === 'left') {
    top = anchor.top
    left = side === 'right' ? anchor.right + offset : anchor.left - offset - size.width
    if (side === 'right' && left + size.width > viewport.width - MARGIN) {
      left = anchor.left - offset - size.width
      finalSide = 'left'
    } else if (side === 'left' && left < MARGIN) {
      left = anchor.right + offset
      finalSide = 'right'
    }
  } else {
    left =
      align === 'start'
        ? anchor.left
        : align === 'end'
          ? anchor.right - size.width
          : anchor.left + anchor.width / 2 - size.width / 2
    top = side === 'top' ? anchor.top - offset - size.height : anchor.bottom + offset
    if (side === 'bottom' && top + size.height > viewport.height - MARGIN) {
      const above = anchor.top - offset - size.height
      if (above >= MARGIN) {
        top = above
        finalSide = 'top'
      }
    } else if (side === 'top' && top < MARGIN) {
      top = anchor.bottom + offset
      finalSide = 'bottom'
    }
  }
  left = Math.min(Math.max(MARGIN, left), Math.max(MARGIN, viewport.width - size.width - MARGIN))
  top = Math.min(Math.max(MARGIN, top), Math.max(MARGIN, viewport.height - size.height - MARGIN))
  return { top, left, side: finalSide }
}

/**
 * Floating panel anchored to an element or a point, on a thick material. Closes on Escape and
 * outside clicks; focus moves in on open and back on close.
 */
export function Popover(props: PopoverProps) {
  return <AnimatePresence>{props.open && <PopoverContent {...props} />}</AnimatePresence>
}

function PopoverContent({
  onClose,
  anchor,
  placement = 'bottom-start',
  offset = 6,
  children,
  role = 'dialog',
  width,
  className,
  autoFocus = true,
  'aria-label': ariaLabel,
}: PopoverProps) {
  const ref = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<{ top: number; left: number; side: string } | null>(null)
  useFocusScope(ref, { autoFocus })

  const place = useCallback(() => {
    const el = ref.current
    if (!el) return
    let rect
    if (isPoint(anchor)) {
      rect = {
        top: anchor.y,
        bottom: anchor.y,
        left: anchor.x,
        right: anchor.x,
        width: 0,
        height: 0,
      }
    } else {
      const a = anchor.current
      if (!a) return
      rect = a.getBoundingClientRect()
    }
    setPos(
      computePosition(
        rect,
        { width: el.offsetWidth, height: el.offsetHeight },
        placement,
        isPoint(anchor) ? 0 : offset,
        { width: window.innerWidth, height: window.innerHeight },
      ),
    )
  }, [anchor, placement, offset])

  useLayoutEffect(() => {
    place()
  }, [place])

  // Content that grows after opening (cast devices being discovered, async lists) must be placed
  // again, otherwise a popover opened upwards grows past the bottom of the window.
  useEffect(() => {
    const el = ref.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(() => place())
    observer.observe(el)
    return () => observer.disconnect()
  }, [place])

  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node
      if (ref.current?.contains(target)) return
      if (!isPoint(anchor) && anchor.current?.contains(target)) return
      onClose()
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        // Captured on window and stopped there: the page's own Escape (leave the video player,
        // close a sheet) must not run when Escape only dismisses this popover.
        e.preventDefault()
        e.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('keydown', onKey, true)
    window.addEventListener('resize', place)
    window.addEventListener('blur', onClose)
    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('resize', place)
      window.removeEventListener('blur', onClose)
    }
  }, [anchor, onClose, place])

  const origin =
    pos?.side === 'top'
      ? 'bottom'
      : pos?.side === 'left'
        ? 'right'
        : pos?.side === 'right'
          ? 'left'
          : 'top'

  return createPortal(
    <motion.div
      ref={ref}
      role={role}
      aria-label={ariaLabel}
      tabIndex={-1}
      className={clsx('jr-popover', 'jr-material-thick', className)}
      style={
        {
          top: pos?.top ?? -9999,
          left: pos?.left ?? -9999,
          width,
          transformOrigin: origin,
          visibility: pos ? 'visible' : 'hidden',
        } as CSSProperties
      }
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, transition: { duration: 0.12 } }}
      transition={{ duration: 0.16, ease: [0.22, 1, 0.36, 1] }}
    >
      {children}
    </motion.div>,
    document.body,
  )
}
