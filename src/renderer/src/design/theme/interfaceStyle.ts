/**
 * Interface style store: which style layer the app wears (classic, glass, cinema, minimal,
 * compact) and the appearance options on top of it (appearance.ts: transparency, corners, density,
 * text size, sidebar, backdrop, motion, video cards), persisted in localStorage and applied to
 * <html> as `data-style`, `data-*` attributes and CSS variables. The layers themselves are CSS
 * (src/renderer/src/styles/interface-styles.css). Framework-free so it runs before React mounts,
 * and in the PiP windows.
 */
import {
  applyAppearanceToElement,
  resolveAppearance,
  sanitizeOverrides,
  type Appearance,
} from './appearance'

export const INTERFACE_STYLES = ['classic', 'glass', 'cinema', 'minimal', 'compact'] as const
export type InterfaceStyle = (typeof INTERFACE_STYLES)[number]

export const DEFAULT_INTERFACE_STYLE: InterfaceStyle = 'classic'
export const INTERFACE_STYLE_STORAGE_KEYS = {
  style: 'orphyn.interface.style',
  /** Legacy switch (1.1), read once: "on" becomes a transparency of 0. */
  reduceTransparency: 'orphyn.interface.reduceTransparency',
  /** JSON object of the appearance options the user changed (see `Appearance`). */
  appearance: 'orphyn.interface.appearance',
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

function storageRemove(key: string): void {
  try {
    globalThis.localStorage?.removeItem(key)
  } catch {
    // Storage disabled: nothing to remove.
  }
}

export function readStoredInterfaceStyle(): InterfaceStyle {
  const v = storageGet(INTERFACE_STYLE_STORAGE_KEYS.style)
  return isInterfaceStyle(v) ? v : DEFAULT_INTERFACE_STYLE
}

/** The appearance options the user changed, validated field by field. */
export function readStoredOverrides(): Partial<Appearance> {
  const text = storageGet(INTERFACE_STYLE_STORAGE_KEYS.appearance)
  let parsed: unknown
  try {
    parsed = text ? JSON.parse(text) : null
  } catch {
    parsed = null
  }
  const overrides = sanitizeOverrides(parsed)
  if (
    overrides.transparency === undefined &&
    storageGet(INTERFACE_STYLE_STORAGE_KEYS.reduceTransparency) === 'true'
  ) {
    overrides.transparency = 0
  }
  return overrides
}

/** Writes the style and its appearance onto <html>. */
export function applyInterfaceStyleToDocument(style: InterfaceStyle, appearance: Appearance): void {
  if (typeof document === 'undefined') return
  document.documentElement.dataset['style'] = style
  applyAppearanceToElement(appearance)
}

export interface InterfaceStyleState {
  style: InterfaceStyle
  /** Effective appearance: the style's defaults with the user's overrides on top. */
  appearance: Appearance
  /** The options the user changed (empty = all the style's defaults). */
  overrides: Partial<Appearance>
  /** Solid surfaces, no blur (`appearance.transparency === 0`). */
  reduceTransparency: boolean
}

type Listener = () => void

function makeState(style: InterfaceStyle, overrides: Partial<Appearance>): InterfaceStyleState {
  const appearance = resolveAppearance(style, overrides)
  return { style, appearance, overrides, reduceTransparency: appearance.transparency <= 0 }
}

let state: InterfaceStyleState = makeState(DEFAULT_INTERFACE_STYLE, {})
const listeners = new Set<Listener>()

function prefersReducedMotion(): boolean {
  if (state.appearance.motion === 'reduced') return true
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

function sameAppearance(a: Appearance, b: Appearance): boolean {
  return (Object.keys(a) as (keyof Appearance)[]).every((k) => a[k] === b[k])
}

function commit(next: InterfaceStyleState, animate: boolean): void {
  const styleChanged = next.style !== state.style
  const changed =
    styleChanged ||
    !sameAppearance(next.appearance, state.appearance) ||
    JSON.stringify(next.overrides) !== JSON.stringify(state.overrides)
  state = next
  const apply = () => applyInterfaceStyleToDocument(next.style, next.appearance)
  // Only a new style cross-fades; options apply at once (a slider must follow the pointer).
  if (animate && styleChanged) withCrossfade(apply)
  else apply()
  if (changed) listeners.forEach((l) => l())
}

function saveOverrides(overrides: Partial<Appearance>): void {
  storageSet(INTERFACE_STYLE_STORAGE_KEYS.appearance, JSON.stringify(overrides))
  // The legacy key would otherwise turn transparency back off on the next launch.
  storageRemove(INTERFACE_STYLE_STORAGE_KEYS.reduceTransparency)
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
    commit(makeState(style, state.overrides), true)
  },
  /** Changes one or more appearance options (persisted, applied at once). */
  setAppearance(patch: Partial<Appearance>): void {
    const overrides = { ...state.overrides, ...sanitizeOverrides(patch) }
    saveOverrides(overrides)
    commit(makeState(state.style, overrides), false)
  },
  /** "Reset to style defaults": drops every appearance option the user changed. */
  resetAppearance(): void {
    saveOverrides({})
    commit(makeState(state.style, {}), false)
  },
  /** Solid surfaces (transparency 0), or back to a translucent interface. */
  setReduceTransparency(on: boolean): void {
    const overrides: Partial<Appearance> = { ...state.overrides }
    delete overrides.transparency
    if (on) overrides.transparency = 0
    else if (resolveAppearance(state.style, overrides).transparency <= 0) {
      overrides.transparency = 100
    }
    saveOverrides(overrides)
    commit(makeState(state.style, overrides), false)
  },
  /** Re-reads storage (used at startup, by the PiP windows and by tests). */
  reload(): void {
    commit(makeState(readStoredInterfaceStyle(), readStoredOverrides()), false)
  },
}

/** Applies the stored style and appearance to <html> before React renders. */
export function applyStoredInterfaceStyle(): void {
  interfaceStyleStore.reload()
}
