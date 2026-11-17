/**
 * Theme store: mode (system / light / dark), accent colour and "follow artwork colour", persisted
 * in localStorage and applied to <html> as `data-theme`, `data-accent` and the accent variables.
 * Every accent (preset, custom or taken from the artwork) is turned into accessible tokens per
 * theme (`accentTokens`): `--accent` reads at AA as text on every surface, `--accent-fill` carries
 * its own AA text colour. Framework-free so it can run before React mounts (see styles/index.ts)
 * and in the PiP windows.
 */
import { contrast, ensureContrast, luminance, parseHex, toHex, type Rgb } from './color'

export type ThemeMode = 'system' | 'light' | 'dark'
export type ResolvedTheme = 'light' | 'dark'

/** Accent presets, mapped to system colours in tokens.css. `red` is the default. */
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
  mode: 'orphyn.theme.mode',
  accent: 'orphyn.theme.accent',
  /** "true": the accent follows the colour of the artwork on screen. */
  followArtwork: 'orphyn.theme.followArtwork',
  /** Last artwork colour (`#rrggbb`), shared with the PiP windows. */
  artworkColor: 'orphyn.theme.artworkColor',
} as const

/** The preset colours, per theme (same values as the system colours of tokens.css). */
export const ACCENT_PRESET_COLORS: Record<AccentPreset, { light: string; dark: string }> = {
  red: { light: '#fa2d48', dark: '#fa2d48' },
  pink: { light: '#ff2d55', dark: '#ff375f' },
  orange: { light: '#ff9500', dark: '#ff9f0a' },
  yellow: { light: '#ffcc00', dark: '#ffd60a' },
  green: { light: '#34c759', dark: '#30d158' },
  teal: { light: '#30b0c7', dark: '#40c8e0' },
  blue: { light: '#007aff', dark: '#0a84ff' },
  indigo: { light: '#5856d6', dark: '#5e5ce6' },
  purple: { light: '#af52de', dark: '#bf5af2' },
  graphite: { light: '#8e8e93', dark: '#8e8e93' },
}

/**
 * The surfaces accent text has to read on, per theme: the darkest light surface (tinted buttons,
 * grouped lists, glass over its wash) and the lightest dark one (elevated sheets, tinted buttons).
 */
const TEXT_SURFACE: Record<ResolvedTheme, Rgb> = {
  light: [229, 229, 234],
  dark: [58, 58, 60],
}
/** The window background a filled control sits on, per theme. */
const WINDOW: Record<ResolvedTheme, Rgb> = {
  light: [255, 255, 255],
  dark: [29, 29, 31],
}
const WHITE: Rgb = [255, 255, 255]
const BLACK: Rgb = [0, 0, 0]
/** WCAG AA for body text. */
export const AA = 4.5

export interface AccentTokens {
  /** Accent for text, icons, rings: AA on the theme's surfaces. */
  accent: string
  /** Text on `accent` (palette rows, badges). */
  accentContrast: string
  /** Accent for filled controls (buttons, selected rows, toggles). */
  fill: string
  /** Text on `fill`: AA. */
  fillContrast: string
}

/**
 * Accessible accent tokens for a colour in a theme. The hue is kept; only lightness moves, and only
 * as far as AA needs: darker in light mode, lighter for text in dark mode. Bright colours (yellow)
 * keep their fill with black text; others get white text on a fill dark enough for it.
 */
export function accentTokens(color: string, theme: ResolvedTheme): AccentTokens {
  const rgb = parseHex(color) ?? (parseHex(ACCENT_PRESET_COLORS.red[theme]) as Rgb)
  const text = ensureContrast(rgb, TEXT_SURFACE[theme], AA, theme === 'light')
  const textContrast = contrast(text, WHITE) >= contrast(text, BLACK) ? WHITE : BLACK
  // A filled control must stand out from the window (3:1, non-text contrast)...
  const visible = ensureContrast(rgb, WINDOW[theme], 3, theme === 'light')
  // ...and carry AA text: black on bright hues (yellow, a white custom colour), white otherwise.
  const bright = luminance(rgb) > 0.5
  const fill = bright
    ? ensureContrast(visible, BLACK, AA, false)
    : ensureContrast(visible, WHITE, AA, true)
  return {
    accent: toHex(text),
    accentContrast: toHex(textContrast),
    fill: toHex(fill),
    fillContrast: bright ? '#000000' : '#ffffff',
  }
}

/** The raw colour of an accent in a theme (`#rrggbb`). */
export function accentHex(accent: AccentColor, theme: ResolvedTheme): string {
  return HEX.test(accent) ? accent : ACCENT_PRESET_COLORS[accent as AccentPreset][theme]
}

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

export function readStoredFollowArtwork(): boolean {
  return storageGet(THEME_STORAGE_KEYS.followArtwork) === 'true'
}

export function readStoredArtworkColor(): string | null {
  const v = storageGet(THEME_STORAGE_KEYS.artworkColor)
  return v && HEX.test(v) ? v.toLowerCase() : null
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

/**
 * The CSS variables that carry an accent onto <html>: one set per theme, picked by
 * `[data-theme]` in tokens.css (so a subtree forced to dark, like the HeroBanner, gets the dark set).
 */
export function accentProperties(
  accent: AccentColor,
  artworkColor: string | null = null,
): Record<string, string> {
  const out: Record<string, string> = {}
  for (const theme of ['light', 'dark'] as const) {
    const t = accentTokens(artworkColor ?? accentHex(accent, theme), theme)
    out[`--accent-${theme}`] = t.accent
    out[`--accent-contrast-${theme}`] = t.accentContrast
    out[`--accent-fill-${theme}`] = t.fill
    out[`--accent-fill-contrast-${theme}`] = t.fillContrast
  }
  return out
}

/** Writes the theme onto <html>. `artworkColor` replaces the accent while following the artwork. */
export function applyThemeToDocument(
  resolved: ResolvedTheme,
  accent: AccentColor,
  artworkColor: string | null = null,
): void {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  root.dataset['theme'] = resolved
  if (HEX.test(accent)) delete root.dataset['accent']
  else root.dataset['accent'] = accent
  if (artworkColor) root.dataset['accentSource'] = 'artwork'
  else delete root.dataset['accentSource']
  for (const [k, v] of Object.entries(accentProperties(accent, artworkColor))) {
    root.style.setProperty(k, v)
  }
}

/** Tells the native window (vibrancy, Mica, caption buttons) about the theme. Safe outside the app. */
function syncNative(mode: ThemeMode, resolved: ResolvedTheme): void {
  const api = typeof window !== 'undefined' ? window.orphyn?.window : undefined
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
  /** The accent follows the colour of the artwork on screen (when there is one). */
  followArtwork: boolean
  /** Colour of the artwork on screen, `null` when none yet (set by the shell). */
  artworkColor: string | null
}

type Listener = () => void

let state: ThemeState = {
  mode: DEFAULT_MODE,
  resolvedTheme: 'light',
  accent: DEFAULT_ACCENT,
  followArtwork: false,
  artworkColor: null,
}
const listeners = new Set<Listener>()

function commit(next: ThemeState): void {
  const changed =
    next.mode !== state.mode ||
    next.resolvedTheme !== state.resolvedTheme ||
    next.accent !== state.accent ||
    next.followArtwork !== state.followArtwork ||
    next.artworkColor !== state.artworkColor
  state = next
  applyThemeToDocument(
    next.resolvedTheme,
    next.accent,
    next.followArtwork ? next.artworkColor : null,
  )
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
  /** The accent follows the artwork on screen (persisted). */
  setFollowArtwork(on: boolean): void {
    storageSet(THEME_STORAGE_KEYS.followArtwork, on ? 'true' : 'false')
    commit({ ...state, followArtwork: on })
  },
  /**
   * Colour of the artwork on screen (`#rrggbb`; anything else keeps the last one). Set by the
   * shell; stored so the PiP windows follow it too.
   */
  setArtworkColor(color: string | null): void {
    if (color === null || !HEX.test(color)) return
    const next = color.toLowerCase()
    if (next === state.artworkColor) return
    storageSet(THEME_STORAGE_KEYS.artworkColor, next)
    commit({ ...state, artworkColor: next })
  },
  /** Re-reads storage and the OS preference (used at startup and by tests). */
  reload(): void {
    const mode = readStoredMode()
    commit({
      mode,
      resolvedTheme: resolveTheme(mode),
      accent: readStoredAccent(),
      followArtwork: readStoredFollowArtwork(),
      artworkColor: readStoredArtworkColor(),
    })
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
