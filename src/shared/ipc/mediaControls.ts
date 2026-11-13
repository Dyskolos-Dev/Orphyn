/**
 * OS media controls: Windows SMTC, macOS Now Playing, Linux MPRIS (Tauri shell only, filled by
 * the media-controls migration task; Electron gets them from Chromium's `navigator.mediaSession`).
 * Optional in `OrphynApi`.
 */
export const MEDIA_CONTROLS_CHANNELS = {
  /** Main -> renderer event, payload: {@link MediaControlAction}. */
  action: 'orphyn:media-controls:action',
} as const

export interface MediaControlsMetadata {
  title: string
  artist?: string
  album?: string
  /** Artwork URL (the native side may download it). */
  artworkUrl?: string
  durationSec?: number
}

export interface MediaControlsPlayback {
  status: 'playing' | 'paused' | 'stopped'
  positionSec?: number
}

export type MediaControlAction =
  | { type: 'play' | 'pause' | 'toggle' | 'next' | 'previous' | 'stop' }
  | { type: 'seek'; positionSec: number }
  | { type: 'seekBy'; offsetSec: number }

export interface MediaControlsApi {
  setMetadata(metadata: MediaControlsMetadata): Promise<void>
  setPlayback(playback: MediaControlsPlayback): Promise<void>
  clear(): Promise<void>
  onAction(listener: (action: MediaControlAction) => void): () => void
}
