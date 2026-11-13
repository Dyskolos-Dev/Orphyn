/**
 * Interface style store: which style layer the app wears (classic, glass, cinema, minimal,
 * compact) and the "Reduce transparency" switch, persisted in localStorage and applied to <html>
 * as `data-style` and `data-reduce-transparency`. The layers themselves are CSS
 * (src/renderer/src/styles/interface-styles.css). Framework-free so it runs before React mounts.
 */

export const INTERFACE_STYLES = ['classic', 'glass', 'cinema', 'minimal', 'compact'] as const
export type InterfaceStyle = (typeof INTERFACE_STYLES)[number]

export const DEFAULT_INTERFACE_STYLE: InterfaceStyle = 'classic'
export const INTERFACE_STYLE_STORAGE_KEYS = {
  style: 'orphyn.interface.style',
  reduceTransparency: 'orphyn.interface.reduceTransparency',
} as const

/** Length of the cross-fade when the style changes. */
export const STYLE_CROSSFADE_MS = 220

export function isInterfaceStyle(v: unknown): v is InterfaceStyle {
  return typeof v === 'string' && (INTERFACE_STYLES as readonly string[]).includes(v)
}

function storageGet(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null
  } catch {
    return null
  }
}

function storageSet(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value)
  } catch {
    // Storage full or disabled: the choice still applies for this session.
  }
}

export function readStoredInterfaceStyle(): InterfaceStyle {
  const v = storageGet(INTERFACE_STYLE_STORAGE_KEYS.style)
  return isInterfaceStyle(v) ? v : DEFAULT_INTERFACE_STYLE
}

export function readStoredReduceTransparency(): boolean {
  return storageGet(INTERFACE_STYLE_STORAGE_KEYS.reduceTransparency) === 'true'
}

/** Writes the style onto <html>. */
export function applyInterfaceStyleToDocument(
  style: InterfaceStyle,
  reduceTransparency: boolean,
): void {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  root.dataset['style'] = style
  if (reduceTransparency) root.dataset['reduceTransparency'] = 'true'
  else delete root.dataset['reduceTransparency']
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  )
}

type ViewTransitionDocument = Document & {
  startViewTransition?: (update: () => void) => unknown
}

/**
 * Runs `update` inside a short cross-fade (View Transitions) when the engine has it and motion is
 * allowed; otherwise applies it at once. Either way the DOM is updated synchronously enough that
 * the change is instant for the user.
 */
function withCrossfade(update: () => void): void {
  const doc = typeof document !== 'undefined' ? (document as ViewTransitionDocument) : undefined
  if (!doc?.startViewTransition || prefersReducedMotion()) {
    update()
    return
  }
  doc.documentElement.dataset['styleTransition'] = 'true'
  try {
    const transition = doc.startViewTransition(update) as { finished?: Promise<unknown> } | null
    const done = () => delete doc.documentElement.dataset['styleTransition']
    if (transition?.finished) void transition.finished.then(done, done)
    else done()
  } catch {
    delete doc.documentElement.dataset['styleTransition']
    update()
  }
}

export interface InterfaceStyleState {
  style: InterfaceStyle
  reduceTransparency: boolean
}

type Listener = () => void

let state: InterfaceStyleState = {
  style: DEFAULT_INTERFACE_STYLE,
  reduceTransparency: false,
}
const listeners = new Set<Listener>()

function commit(next: InterfaceStyleState, animate: boolean): void {
  const changed = next.style !== state.style || next.reduceTransparency !== state.reduceTransparency
  state = next
  const apply = () => applyInterfaceStyleToDocument(next.style, next.reduceTransparency)
  if (animate && changed) withCrossfade(apply)
  else apply()
  if (changed) listeners.forEach((l) => l())
}

export const interfaceStyleStore = {
  getState: (): InterfaceStyleState => state,
  subscribe(listener: Listener): () => void {
    listeners.add(listener)
    return () => {
      listeners.delete(listener)
    }
  },
  setStyle(style: InterfaceStyle): void {
    if (!isInterfaceStyle(style)) return
    storageSet(INTERFACE_STYLE_STORAGE_KEYS.style, style)
    commit({ ...state, style }, true)
  },
  setReduceTransparency(on: boolean): void {
    storageSet(INTERFACE_STYLE_STORAGE_KEYS.reduceTransparency, on ? 'true' : 'false')
    commit({ ...state, reduceTransparency: on }, true)
  },
  /** Re-reads storage (used at startup and by tests). */
  reload(): void {
    commit(
      { style: readStoredInterfaceStyle(), reduceTransparency: readStoredReduceTransparency() },
      false,
    )
  },
}

/** Applies the stored style to <html> before React renders. */
export function applyStoredInterfaceStyle(): void {
  interfaceStyleStore.reload()
}
