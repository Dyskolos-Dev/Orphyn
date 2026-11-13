import { MotionConfig } from 'framer-motion'
import { useEffect, useSyncExternalStore, type ReactNode } from 'react'
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

/**
 * Applies the persisted theme and interface style, follows the OS appearance in `system` mode and
 * makes framer-motion honour "Reduce motion". Mounted once by the app shell; `useTheme()` and
 * `useInterfaceStyle()` also work outside it.
 */
export function ThemeProvider({ children }: ThemeProviderProps) {
  useEffect(() => {
    themeStore.reload()
    interfaceStyleStore.reload()
    return watchSystemTheme()
  }, [])
  return <MotionConfig reducedMotion="user">{children}</MotionConfig>
}

export interface UseThemeResult extends ThemeState {
  setMode(mode: ThemeMode): void
  setAccent(accent: AccentColor): void
}

/** Current theme + setters. Changes apply instantly and persist in localStorage. */
export function useTheme(): UseThemeResult {
  const state = useSyncExternalStore(themeStore.subscribe, themeStore.getState, themeStore.getState)
  return { ...state, setMode: themeStore.setMode, setAccent: themeStore.setAccent }
}

export interface UseInterfaceStyleResult extends InterfaceStyleState {
  setStyle(style: InterfaceStyle): void
  setReduceTransparency(on: boolean): void
}

/**
 * Current interface style (classic, glass, cinema, minimal, compact) and "Reduce transparency".
 * Changes apply instantly with a short cross-fade and persist in localStorage (keys in
 * `INTERFACE_STYLE_STORAGE_KEYS`); they combine with the mode and accent of `useTheme()`.
 */
export function useInterfaceStyle(): UseInterfaceStyleResult {
  const state = useSyncExternalStore(
    interfaceStyleStore.subscribe,
    interfaceStyleStore.getState,
    interfaceStyleStore.getState,
  )
  return {
    ...state,
    setStyle: interfaceStyleStore.setStyle,
    setReduceTransparency: interfaceStyleStore.setReduceTransparency,
  }
}
