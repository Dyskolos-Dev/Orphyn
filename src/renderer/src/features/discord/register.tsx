import { useEffect } from 'react'
import { registerShellExtension } from '@renderer/shell'
import { startDiscordSync } from './sync'

/** Always mounted: mirrors the music and video players to Discord Rich Presence. */
function DiscordBackground() {
  useEffect(() => {
    if (!window.lumen?.discord) return
    return startDiscordSync()
  }, [])
  return null
}

registerShellExtension({ id: 'discord', background: DiscordBackground })
