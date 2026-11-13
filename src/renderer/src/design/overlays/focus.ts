import { useEffect, useLayoutEffect, useState, type RefObject } from 'react'

export const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export function focusableIn(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => !el.hasAttribute('hidden') && el.getAttribute('aria-hidden') !== 'true',
  )
}

/**
 * While mounted: moves focus into `ref` (initial element, first focusable, or the container),
 * keeps Tab / Shift+Tab inside it when `trap` is set, and gives focus back on unmount.
 */
export function useFocusScope(
  ref: RefObject<HTMLElement | null>,
  {
    trap = false,
    initialFocus,
    autoFocus = true,
  }: { trap?: boolean; initialFocus?: RefObject<HTMLElement | null>; autoFocus?: boolean },
): void {
  // Captured during the first render, before children's refs or effects can move focus.
  const [previously] = useState(() =>
    typeof document === 'undefined' ? null : (document.activeElement as HTMLElement | null),
  )
  useLayoutEffect(() => {
    const root = ref.current
    if (root && autoFocus) {
      // Elements marked data-skip-autofocus (close buttons) are only a last resort.
      const items = focusableIn(root)
      const target =
        initialFocus?.current ??
        items.find((el) => !el.hasAttribute('data-skip-autofocus')) ??
        items[0] ??
        root
      target.focus({ preventScroll: true })
    }
    return () => {
      if (previously && typeof previously.focus === 'function' && previously.isConnected) {
        previously.focus({ preventScroll: true })
      }
    }
    // Runs once per mount: the scope is created when the overlay opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    if (!trap) return
    const onKey = (e: KeyboardEvent) => {
      const root = ref.current
      if (e.key !== 'Tab' || !root) return
      const items = focusableIn(root)
      if (items.length === 0) {
        e.preventDefault()
        root.focus()
        return
      }
      const first = items[0]
      const last = items[items.length - 1]
      const active = document.activeElement
      if (e.shiftKey && (active === first || !root.contains(active))) {
        e.preventDefault()
        last?.focus()
      } else if (!e.shiftKey && (active === last || !root.contains(active))) {
        e.preventDefault()
        first?.focus()
      }
    }
    document.addEventListener('keydown', onKey, true)
    return () => document.removeEventListener('keydown', onKey, true)
  }, [ref, trap])
}
