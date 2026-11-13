import i18n from 'i18next'
import { toast } from '@renderer/design'
import {
  describeCurrentTrackForCast,
  musicPlayer,
  setMusicOutputName,
  type AudioOutput,
} from '@renderer/features/music'
import type { CastDevice } from '@shared/cast'
import { getCastApi } from './api'
import { RemoteAudioOutput } from './RemoteAudioOutput'
import { useCastState } from './store'

let local: AudioOutput | null = null
let remote: RemoteAudioOutput | null = null

/** Device the music currently plays on (null = this computer). */
export const currentMusicDeviceId = (): string | null => remote?.deviceId ?? null

function showTarget(device: CastDevice | null): void {
  useCastState.setState({
    target: device
      ? { deviceId: device.id, name: device.name, kind: device.kind, scope: 'music' }
      : null,
  })
  setMusicOutputName(device?.name ?? null)
}

/**
 * Moves the music to `device`, or back to this computer with null. The engine carries the current
 * track, position and play/pause intent over (`setOutput`). Going back to local leaves the music
 * paused at the position the device had reached.
 */
export async function switchMusicOutput(device: CastDevice | null): Promise<void> {
  const cast = getCastApi()
  if (!cast) throw new Error('Casting is not available')

  if (!device) {
    const previous = remote
    if (!previous || !local) return
    remote = null
    musicPlayer.pause() // comes back paused, at the remote position
    musicPlayer.setOutput(local)
    local = null
    showTarget(null)
    previous.dispose()
    await previous.stopRemote()
    return
  }

  if (remote?.deviceId === device.id) return
  const previous = remote
  if (!previous) local = musicPlayer.getOutput()
  const next = new RemoteAudioOutput({
    cast,
    deviceId: device.id,
    describe: () => describeCurrentTrackForCast(),
    onFailure: (message) => {
      if (remote !== next) return
      toast({
        title: i18n.t('cast:deviceFailed', { device: device.name }),
        description: message,
        tone: 'error',
      })
      void switchMusicOutput(null)
    },
  })
  remote = next
  musicPlayer.setOutput(next)
  showTarget(device)
  if (previous) {
    previous.dispose()
    await previous.stopRemote()
  }
}
