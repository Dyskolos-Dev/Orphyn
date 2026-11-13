import { MotionConfig } from 'framer-motion'
import { useEffect, useSyncExternalStore, type ReactNode } from 'react'
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
 * Applies the persisted theme, follows the OS appearance in `system` mode and makes framer-motion
 * honour "Reduce motion". Mounted once by the app shell; `useTheme()` also works outside it.
 */
export function ThemeProvider({ children }: ThemeProviderProps) {
  useEffect(() => {
    themeStore.reload()
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
