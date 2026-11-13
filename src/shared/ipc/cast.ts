/** Cast IPC contract (owner: cast agent). */
import type { CastDevice, CastMedia, CastSessionState } from '../cast'

export const CAST_CHANNELS = {
  startDiscovery: 'cast:start-discovery',
  stopDiscovery: 'cast:stop-discovery',
  listDevices: 'cast:list-devices',
  getSession: 'cast:get-session',
  load: 'cast:load',
  play: 'cast:play',
  pause: 'cast:pause',
  seek: 'cast:seek',
  stop: 'cast:stop',
  setVolume: 'cast:set-volume',
  setMuted: 'cast:set-muted',
  disconnect: 'cast:disconnect',
  // main -> renderer events
  devicesChanged: 'cast:devices-changed',
  sessionChanged: 'cast:session-changed',
  ended: 'cast:ended',
  error: 'cast:error',
} as const

export interface CastEndedEvent {
  deviceId: string
  media: CastMedia | null
}

export interface CastErrorEvent {
  deviceId: string | null
  message: string
}

export type Unsubscribe = () => void

export interface CastApi {
  startDiscovery(): Promise<void>
  stopDiscovery(): Promise<void>
  listDevices(): Promise<CastDevice[]>
  getSession(deviceId: string): Promise<CastSessionState | null>
  load(deviceId: string, media: CastMedia): Promise<void>
  play(deviceId: string): Promise<void>
  pause(deviceId: string): Promise<void>
  seek(deviceId: string, positionSec: number): Promise<void>
  stop(deviceId: string): Promise<void>
  setVolume(deviceId: string, volume: number): Promise<void>
  setMuted(deviceId: string, muted: boolean): Promise<void>
  disconnect(deviceId: string): Promise<void>
  onDevicesChanged(cb: (devices: CastDevice[]) => void): Unsubscribe
  onSessionChanged(cb: (state: CastSessionState) => void): Unsubscribe
  /** The current track finished playing on the device: advance the queue. */
  onEnded(cb: (event: CastEndedEvent) => void): Unsubscribe
  onError(cb: (event: CastErrorEvent) => void): Unsubscribe
}
