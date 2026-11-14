import { useEffect, useState } from 'react'
import type { UpdaterState } from '@shared/ipc/updater'

/** Live updater state from the native core; null outside the app. */
export function useUpdater(): UpdaterState | null {
  const [state, setState] = useState<UpdaterState | null>(null)
  useEffect(() => {
    const api = window.orphyn?.updater
    if (!api) return
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
  return state
}
