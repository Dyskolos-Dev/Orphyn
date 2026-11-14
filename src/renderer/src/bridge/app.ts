/**
 * Application actions as keyboard shortcuts (there is no native menu). Ctrl+R / Cmd+R refreshes the library (instead of reloading the page),
 * Ctrl+, / Cmd+, opens the settings.
 */
import type { AppApi } from '@shared/ipc/app'

type Action = 'refresh' | 'openSettings'

const listeners: Record<Action, Set<() => void>> = {
  refresh: new Set(),
  openSettings: new Set(),
}

function add(action: Action, listener: () => void): () => void {
  listeners[action].add(listener)
  return () => {
    listeners[action].delete(listener)
  }
}

export function shortcutAction(e: KeyboardEvent, platform: string): Action | null {
  const mod = platform === 'darwin' ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey
  if (!mod || e.altKey || e.shiftKey) return null
  if (e.key === 'r' || e.key === 'R') return 'refresh'
  if (e.key === ',') return 'openSettings'
  return null
}

export function installAppShortcuts(platform: string): void {
  window.addEventListener(
    'keydown',
    (e) => {
      const action = shortcutAction(e, platform)
      if (!action) return
      e.preventDefault()
      listeners[action].forEach((l) => l())
    },
    { capture: true },
  )
}

export const appApi: AppApi = {
  onRefresh: (l) => add('refresh', l),
  onOpenSettings: (l) => add('openSettings', l),
}
