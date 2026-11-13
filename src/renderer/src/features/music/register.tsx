import { useEffect } from 'react'
import { registerShellExtension } from '@renderer/shell'
import { MiniPlayer } from './components/MiniPlayer'
import { NowPlaying } from './components/NowPlaying'
import { startPlayerEngine } from './player/instance'
import { installMediaSession } from './player/mediaSession'

/** Always mounted by the shell: starts the audio engine, publishes OS media controls, hosts the sheet. */
function MusicBackground() {
  useEffect(() => {
    startPlayerEngine()
    return installMediaSession()
  }, [])
  return <NowPlaying />
}

// Persistent bottom bar + background work, registered when the music routes are imported.
registerShellExtension({ id: 'music', nowPlaying: MiniPlayer, background: MusicBackground })
