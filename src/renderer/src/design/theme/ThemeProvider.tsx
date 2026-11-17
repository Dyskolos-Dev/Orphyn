import { MotionConfig } from 'framer-motion'
import { useEffect, useSyncExternalStore, type ReactNode } from 'react'
import type { Appearance } from './appearance'
import {
  interfaceStyleStore,
  type InterfaceStyle,
  type InterfaceStyleState,
} from './interfaceStyle'
import {
  themeStore,
  watchSystemTheme,
  type AccentColor,
  type ThemeMode,
  type ThemeState,
} from './theme'

export interface ThemeProviderProps {
  children?: ReactNode
}

function useInterfaceStyleState(): InterfaceStyleState {
  return useSyncExternalStore(
    interfaceStyleStore.subscribe,
    interfaceStyleStore.getState,
    interfaceStyleStore.getState,
  )
}

/**
 * Applies the persisted theme, interface style and appearance, follows the OS appearance in
 * `system` mode and makes framer-motion honour "Reduce motion" (the OS setting, or the app's own
 * Motion option). Mounted once by the app shell; the hooks also work outside it.
 */
export function ThemeProvider({ children }: ThemeProviderProps) {
  useEffect(() => {
    themeStore.reload()
    interfaceStyleStore.reload()
    return watchSystemTheme()
  }, [])
  const { appearance } = useInterfaceStyleState()
  return (
    <MotionConfig reducedMotion={appearance.motion === 'reduced' ? 'always' : 'user'}>
      {children}
    </MotionConfig>
  )
}

export interface UseThemeResult extends ThemeState {
  setMode(mode: ThemeMode): void
  setAccent(accent: AccentColor): void
  setFollowArtwork(on: boolean): void
}

/** Current theme + setters. Changes apply instantly and persist in localStorage. */
export function useTheme(): UseThemeResult {
  const state = useSyncExternalStore(themeStore.subscribe, themeStore.getState, themeStore.getState)
  return {
    ...state,
    setMode: themeStore.setMode,
    setAccent: themeStore.setAccent,
    setFollowArtwork: themeStore.setFollowArtwork,
  }
}

export interface UseInterfaceStyleResult extends InterfaceStyleState {
  setStyle(style: InterfaceStyle): void
  setReduceTransparency(on: boolean): void
  /** Changes appearance options; applied at once on <html> and persisted. */
  setAppearance(patch: Partial<Appearance>): void
  /** Back to the current style's defaults for every appearance option. */
  resetAppearance(): void
}

/**
 * Current interface style (classic, glass, cinema, minimal, compact) and its appearance options
 * (transparency, corners, density, text size, sidebar, backdrop, motion, video cards). Changes
 * apply instantly (a new style with a short cross-fade) and persist in localStorage (keys in
 * `INTERFACE_STYLE_STORAGE_KEYS`); they combine with the mode and accent of `useTheme()`.
 */
export function useInterfaceStyle(): UseInterfaceStyleResult {
  const state = useInterfaceStyleState()
  return {
    ...state,
    setStyle: interfaceStyleStore.setStyle,
    setReduceTransparency: interfaceStyleStore.setReduceTransparency,
    setAppearance: interfaceStyleStore.setAppearance,
    resetAppearance: interfaceStyleStore.resetAppearance,
  }
}

/** The effective appearance options only (re-renders when one changes). */
export function useAppearance(): Appearance {
  return useInterfaceStyleState().appearance
}
