/** Device-agnostic casting model shared with the renderer (owner: cast agent). */

export type CastKind = 'chromecast' | 'upnp' | 'airplay'

export interface CastCapabilities {
  audio: boolean
  video: boolean
}

export interface CastDevice {
  /** Stable id, prefixed by kind (e.g. `chromecast:<uuid>`, `upnp:<udn>`). */
  id: string
  name: string
  kind: CastKind
  model: string
  manufacturer: string
  capabilities: CastCapabilities
  /** IP address (v4 preferred). */
  address: string
  port: number
  /** True for WiiM / LinkPlay streamers (reached through UPnP). */
  wiim: boolean
}

export type CastMediaKind = 'audio' | 'video'

export interface CastMedia {
  /** Absolute stream URL reachable from the device (includes api_key). */
  url: string
  mimeType: string
  kind: CastMediaKind
  title: string
  artist?: string
  album?: string
  artworkUrl?: string
  durationSec?: number
  /** Start offset in seconds. */
  startPositionSec?: number
}

export type CastStatus = 'idle' | 'connecting' | 'playing' | 'paused' | 'buffering' | 'error'

export interface CastSessionState {
  deviceId: string
  status: CastStatus
  positionSec: number
  durationSec: number
  /** 0..1 */
  volume: number
  muted: boolean
  media: CastMedia | null
  error?: string
}

export type CastCommand =
  | { type: 'load'; media: CastMedia }
  | { type: 'play' }
  | { type: 'pause' }
  | { type: 'seek'; positionSec: number }
  | { type: 'stop' }
  | { type: 'setVolume'; volume: number }
  | { type: 'setMuted'; muted: boolean }
  | { type: 'disconnect' }

export function createIdleState(deviceId: string): CastSessionState {
  return {
    deviceId,
    status: 'idle',
    positionSec: 0,
    durationSec: 0,
    volume: 1,
    muted: false,
    media: null,
  }
}
