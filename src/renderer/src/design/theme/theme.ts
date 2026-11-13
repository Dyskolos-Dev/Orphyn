/**
 * Theme store: mode (system / light / dark) + accent colour, persisted in localStorage and applied
 * to <html> as `data-theme`, `data-accent` (or an inline `--accent` for custom colours).
 * Framework-free so it can run before React mounts (see styles/index.ts).
 */

export type ThemeMode = 'system' | 'light' | 'dark'
export type ResolvedTheme = 'light' | 'dark'

/** Accent presets, mapped to Apple system colours in tokens.css. `red` is Apple Music's red. */
export const ACCENT_PRESETS = [
  'red',
  'pink',
  'orange',
  'yellow',
  'green',
  'teal',
  'blue',
  'indigo',
  'purple',
  'graphite',
] as const
export type AccentPreset = (typeof ACCENT_PRESETS)[number]
/** A preset name or a custom `#rrggbb` colour. */
export type AccentColor = AccentPreset | `#${string}`

export const THEME_MODES: readonly ThemeMode[] = ['system', 'light', 'dark']
export const DEFAULT_MODE: ThemeMode = 'system'
export const DEFAULT_ACCENT: AccentColor = 'red'
export const THEME_STORAGE_KEYS = {
  mode: 'jellyra.theme.mode',
  accent: 'jellyra.theme.accent',
} as const

const HEX = /^#[0-9a-f]{6}$/i
const DARK_QUERY = '(prefers-color-scheme: dark)'

export function isThemeMode(v: unknown): v is ThemeMode {
  return v === 'system' || v === 'light' || v === 'dark'
}

export function isAccentColor(v: unknown): v is AccentColor {
  return typeof v === 'string' && ((ACCENT_PRESETS as readonly string[]).includes(v) || HEX.test(v))
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

export function readStoredMode(): ThemeMode {
  const v = storageGet(THEME_STORAGE_KEYS.mode)
  return isThemeMode(v) ? v : DEFAULT_MODE
}

export function readStoredAccent(): AccentColor {
  const v = storageGet(THEME_STORAGE_KEYS.accent)
  return isAccentColor(v) ? v : DEFAULT_ACCENT
}

function darkQuery(): MediaQueryList | null {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia(DARK_QUERY)
    : null
}

export function systemPrefersDark(): boolean {
  return darkQuery()?.matches ?? false
}

export function resolveTheme(mode: ThemeMode, prefersDark = systemPrefersDark()): ResolvedTheme {
  if (mode === 'system') return prefersDark ? 'dark' : 'light'
  return mode
}

/** Black or white text on a custom accent, by relative luminance. */
export function contrastOn(hex: string): '#000000' | '#ffffff' {
  const n = parseInt(hex.slice(1), 16)
  const lin = (c: number) => {
    const s = c / 255
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }
  const l = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255)
  return l > 0.4 ? '#000000' : '#ffffff'
}

/** Writes the theme onto <html>. */
export function applyThemeToDocument(resolved: ResolvedTheme, accent: AccentColor): void {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  root.dataset['theme'] = resolved
  if (HEX.test(accent)) {
    delete root.dataset['accent']
    root.style.setProperty('--accent', accent)
    root.style.setProperty('--accent-contrast', contrastOn(accent))
  } else {
    root.dataset['accent'] = accent
    root.style.removeProperty('--accent')
    root.style.removeProperty('--accent-contrast')
  }
}

/** Tells the native window (vibrancy, Mica, caption buttons) about the theme. Safe outside Electron. */
function syncNative(mode: ThemeMode, resolved: ResolvedTheme): void {
  const api = typeof window !== 'undefined' ? window.jellyra?.window : undefined
  if (!api) return
  void api.setAppearance?.(mode).catch(() => undefined)
  void api
    .setTitleBarOverlay?.({
      color: '#00000000',
      symbolColor: resolved === 'dark' ? '#ffffff' : '#000000',
    })
    .catch(() => undefined)
}

export interface ThemeState {
  mode: ThemeMode
  resolvedTheme: ResolvedTheme
  accent: AccentColor
}

type Listener = () => void

let state: ThemeState = {
  mode: DEFAULT_MODE,
  resolvedTheme: 'light',
  accent: DEFAULT_ACCENT,
}
const listeners = new Set<Listener>()

function commit(next: ThemeState): void {
  const changed =
    next.mode !== state.mode ||
    next.resolvedTheme !== state.resolvedTheme ||
    next.accent !== state.accent
  state = next
  applyThemeToDocument(next.resolvedTheme, next.accent)
  if (changed) {
    syncNative(next.mode, next.resolvedTheme)
    listeners.forEach((l) => l())
  }
}

export const themeStore = {
  getState: (): ThemeState => state,
  subscribe(listener: Listener): () => void {
    listeners.add(listener)
    return () => {
      listeners.delete(listener)
    }
  },
  setMode(mode: ThemeMode): void {
    if (!isThemeMode(mode)) return
    storageSet(THEME_STORAGE_KEYS.mode, mode)
    commit({ ...state, mode, resolvedTheme: resolveTheme(mode) })
  },
  setAccent(accent: AccentColor): void {
    if (!isAccentColor(accent)) return
    storageSet(THEME_STORAGE_KEYS.accent, accent)
    commit({ ...state, accent })
  },
  /** Re-reads storage and the OS preference (used at startup and by tests). */
  reload(): void {
    const mode = readStoredMode()
    commit({ mode, resolvedTheme: resolveTheme(mode), accent: readStoredAccent() })
    syncNative(state.mode, state.resolvedTheme)
  },
  /** Re-resolves `system` mode after the OS appearance changed. */
  refreshSystem(): void {
    commit({ ...state, resolvedTheme: resolveTheme(state.mode) })
  },
}

/** Applies the stored theme to <html> before React renders. */
export function applyStoredTheme(): void {
  themeStore.reload()
}

/** Follows the OS appearance while in `system` mode. Returns a cleanup function. */
export function watchSystemTheme(): () => void {
  const mq = darkQuery()
  if (!mq) return () => undefined
  const onChange = () => themeStore.refreshSystem()
  mq.addEventListener('change', onChange)
  return () => mq.removeEventListener('change', onChange)
}
