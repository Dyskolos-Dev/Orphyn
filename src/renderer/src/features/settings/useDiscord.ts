import { useEffect, useState } from 'react'
import type { DiscordState } from '@shared/ipc/discord'

/** Live Discord Rich Presence state from the main process; null outside Electron. */
export function useDiscord(): DiscordState | null {
  const [state, setState] = useState<DiscordState | null>(null)
  useEffect(() => {
    const api = window.orphyn?.discord
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
