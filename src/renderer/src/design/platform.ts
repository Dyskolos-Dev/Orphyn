import { useCallback, useEffect, useState } from 'react'
import type { WindowApi, WindowState } from '@shared/ipc/window'

/** `darwin` | `win32` | `linux` in Electron, `web` in a browser or tests. */
export type Platform = 'darwin' | 'win32' | 'linux' | 'web'

function windowApi(): WindowApi | undefined {
  return typeof window !== 'undefined' ? window.orphyn?.window : undefined
}

export function getPlatform(): Platform {
  const p = typeof window !== 'undefined' ? window.orphyn?.platform : undefined
  return p === 'darwin' || p === 'win32' || p === 'linux' ? p : 'web'
}

const INITIAL: WindowState = {
  isFullscreen: false,
  isMaximized: false,
  isFocused: true,
  material: 'none',
}

/** Writes platform / material / fullscreen / focus onto <html> (used by tokens.css). */
export function applyWindowStateToDocument(platform: Platform, s: WindowState): void {
  const d = document.documentElement.dataset
  d['platform'] = platform
  d['material'] = s.material
  d['fullscreen'] = String(s.isFullscreen)
  d['windowFocused'] = String(s.isFocused)
}

export interface UseWindowStateResult extends WindowState {
  platform: Platform
  /** Enter / leave native fullscreen (falls back to the DOM Fullscreen API outside Electron). */
  setFullscreen(fullscreen: boolean): Promise<void>
  toggleMaximize(): Promise<void>
}

/** Live native window state (fullscreen, maximized, focus, material) + controls. */
export function useWindowState(): UseWindowStateResult {
  const [state, setState] = useState<WindowState>(INITIAL)

  useEffect(() => {
    const api = windowApi()
    if (!api) {
      const onFs = () =>
        setState((s) => ({ ...s, isFullscreen: document.fullscreenElement != null }))
      document.addEventListener('fullscreenchange', onFs)
      return () => document.removeEventListener('fullscreenchange', onFs)
    }
    let alive = true
    void api
      .getState()
      .then((s) => alive && setState(s))
      .catch(() => undefined)
    const off = api.onStateChange((s) => setState(s))
    return () => {
      alive = false
      off()
    }
  }, [])

  const setFullscreen = useCallback(async (fullscreen: boolean) => {
    const api = windowApi()
    if (api) return api.setFullscreen(fullscreen)
    if (fullscreen) await document.documentElement.requestFullscreen?.()
    else if (document.fullscreenElement) await document.exitFullscreen?.()
  }, [])

  const toggleMaximize = useCallback(async () => {
    await windowApi()?.toggleMaximize()
  }, [])

  return { ...state, platform: getPlatform(), setFullscreen, toggleMaximize }
}

/** Modifier shown in shortcut hints: ⌘ on macOS, Ctrl elsewhere. */
export function modifierLabel(platform: Platform = getPlatform()): string {
  return platform === 'darwin' ? '⌘' : 'Ctrl+'
}

/** `formatShortcut('R')` -> "⌘R" on macOS, "Ctrl+R" elsewhere. */
export function formatShortcut(key: string, platform: Platform = getPlatform()): string {
  return `${modifierLabel(platform)}${key}`
}
