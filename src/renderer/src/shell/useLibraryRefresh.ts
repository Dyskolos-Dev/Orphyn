import { useIsFetching, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect } from 'react'
import { jfKeys, refreshLibrary, shouldRefreshOnFocus, useSession } from '@renderer/jellyfin'

/** Refresh action of the active server: `refresh()` refetches its data, `fetching` drives the spinner. */
export function useLibraryRefresh(): { refresh: () => void; fetching: boolean } {
  const client = useQueryClient()
  const accountId = useSession((s) => s.account?.id ?? null)
  const fetching =
    useIsFetching({ queryKey: accountId ? jfKeys.all(accountId) : ['jf', '-none-'] }) > 0
  const refresh = useCallback(() => {
    void refreshLibrary(client, accountId)
  }, [client, accountId])
  return { refresh, fetching }
}

/**
 * Global triggers: Cmd+R (macOS) / Ctrl+R / F5 instead of reloading the window, the native menu
 * item, and a refetch when the window comes back after more than 5 minutes in the background.
 */
export function useLibraryRefreshTriggers(platform: string, refresh: () => void): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = platform === 'darwin' ? e.metaKey : e.ctrlKey
      const isR = mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'r'
      if (isR || (e.key === 'F5' && !mod && !e.altKey)) {
        e.preventDefault()
        refresh()
      }
    }
    window.addEventListener('keydown', onKey)
    const offMenu = window.lumen?.app?.onRefresh(refresh)

    let hiddenSince: number | null = null
    const leave = () => (hiddenSince ??= Date.now())
    const back = () => {
      if (shouldRefreshOnFocus(hiddenSince, Date.now())) refresh()
      hiddenSince = null
    }
    const onVisibility = () => (document.visibilityState === 'hidden' ? leave() : back())
    window.addEventListener('blur', leave)
    window.addEventListener('focus', back)
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      window.removeEventListener('keydown', onKey)
      offMenu?.()
      window.removeEventListener('blur', leave)
      window.removeEventListener('focus', back)
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [platform, refresh])
}
