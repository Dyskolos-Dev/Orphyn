/** Cast bridge: one function per Rust command of `src-tauri/src/cast/` (see docs/CASTING.md). */
import type { CastDevice, CastSessionState } from '@shared/cast'
import {
  CAST_CHANNELS as C,
  type CastApi,
  type CastEndedEvent,
  type CastErrorEvent,
} from '@shared/ipc/cast'
import { call, subscribe } from './tauri'

export const castApi: CastApi = {
  startDiscovery: () => call('cast_start_discovery'),
  stopDiscovery: () => call('cast_stop_discovery'),
  listDevices: () => call('cast_list_devices'),
  getSession: (deviceId) => call('cast_get_session', { deviceId }),
  load: (deviceId, media) => call('cast_load', { deviceId, media }),
  play: (deviceId) => call('cast_play', { deviceId }),
  pause: (deviceId) => call('cast_pause', { deviceId }),
  seek: (deviceId, positionSec) => call('cast_seek', { deviceId, positionSec }),
  stop: (deviceId) => call('cast_stop', { deviceId }),
  setVolume: (deviceId, volume) => call('cast_set_volume', { deviceId, volume }),
  setMuted: (deviceId, muted) => call('cast_set_muted', { deviceId, muted }),
  disconnect: (deviceId) => call('cast_disconnect', { deviceId }),
  onDevicesChanged: (cb) => subscribe<CastDevice[]>(C.devicesChanged, cb),
  onSessionChanged: (cb) => subscribe<CastSessionState>(C.sessionChanged, cb),
  onEnded: (cb) => subscribe<CastEndedEvent>(C.ended, cb),
  onError: (cb) => subscribe<CastErrorEvent>(C.error, cb),
}
