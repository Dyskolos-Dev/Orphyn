/**
 * Messages between the main window and the PiP windows. The players stay in the main window; the
 * PiP window only shows a copy of their state and sends commands back. Everything travels as
 * Tauri events (broadcast), so no Rust relay is needed. See `docs/PIP.md`.
 */

export const PIP_EVENTS = {
  /** main -> music PiP: {@link MusicPipState}. */
  musicState: 'orphyn:pip:music-state',
  /** music PiP -> main: {@link MusicPipCommand}. */
  musicCommand: 'orphyn:pip:music-command',
  /** PiP -> main: the PiP window is loaded and wants the current state. */
  ready: 'orphyn:pip:ready',
  /** main -> video PiP: {@link VideoPipStart}. */
  videoStart: 'orphyn:pip:video-start',
  /** video PiP -> main: {@link VideoPipProgress}, about once a second. */
  videoProgress: 'orphyn:pip:video-progress',
} as const

export interface MusicPipState {
  hasTrack: boolean
  trackId?: string
  title: string
  artist: string
  album?: string
  artworkUrl?: string
  playing: boolean
  /** Seconds, at the time `at`. */
  position: number
  duration: number
  favorite: boolean
  /** Sender clock (ms since epoch) when this snapshot was taken. */
  at: number
}

export const EMPTY_MUSIC_PIP: MusicPipState = {
  hasTrack: false,
  title: '',
  artist: '',
  playing: false,
  position: 0,
  duration: 0,
  favorite: false,
  at: 0,
}

export type MusicPipAction = { type: 'snapshot'; state: MusicPipState } | { type: 'clear' }

/** The PiP window's copy of the player. A snapshot older than the one held is dropped. */
export function musicPipReducer(state: MusicPipState, action: MusicPipAction): MusicPipState {
  switch (action.type) {
    case 'clear':
      return EMPTY_MUSIC_PIP
    case 'snapshot':
      return action.state.at >= state.at ? action.state : state
  }
}

/** Position to show at `now`: while playing the bar keeps moving between two snapshots. */
export function extrapolatePosition(state: MusicPipState, now: number): number {
  if (!state.playing) return state.position
  const elapsed = Math.max(0, (now - state.at) / 1000)
  const pos = state.position + elapsed
  return state.duration > 0 ? Math.min(pos, state.duration) : pos
}

export type MusicPipCommand =
  { type: 'toggle' | 'next' | 'previous' | 'favorite' } | { type: 'seek'; positionSec: number }

/** A snapshot is worth sending again when something other than the playing clock changed. */
export function musicPipChanged(a: MusicPipState, b: MusicPipState): boolean {
  return (
    a.hasTrack !== b.hasTrack ||
    a.trackId !== b.trackId ||
    a.title !== b.title ||
    a.artist !== b.artist ||
    a.artworkUrl !== b.artworkUrl ||
    a.playing !== b.playing ||
    a.favorite !== b.favorite ||
    a.duration !== b.duration
  )
}

/** Video PiP (custom window): what the main window hands over. */
export interface VideoPipStart {
  itemId: string
  title: string
  subtitle?: string
  positionSec: number
  audioIndex?: number
  subtitleIndex?: number
}

export interface VideoPipProgress {
  positionSec: number
  durationSec: number
  playing: boolean
  ended: boolean
}
