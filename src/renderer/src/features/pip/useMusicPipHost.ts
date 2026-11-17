import { useEffect, useRef } from 'react'
import { useIsFavorite, useToggleFavorite } from '@renderer/jellyfin'
import { musicPlayer, useCurrentTrack, useMusicPlayer } from '@renderer/features/music/player/api'
import type { PlayerState } from '@renderer/features/music/player/store'
import { currentItem } from '@renderer/features/music/player/queue'
import { trackArt, trackArtist, trackTitle } from '@renderer/features/music/player/track'
import { onPip, sendPip, togglePip, usePipState } from './events'
import {
  ARTWORK_MAX,
  EMPTY_MUSIC_PIP,
  PIP_EVENTS,
  musicPipChanged,
  type MusicPipCommand,
  type MusicPipState,
} from './protocol'

/** While playing, a fresh snapshot is sent at most this often to correct the PiP's own clock. */
const RESYNC_MS = 2000

/** The music PiP's view of the player. */
export function buildMusicPipState(s: PlayerState, favorite: boolean, now: number): MusicPipState {
  const track = currentItem(s)?.track
  if (!track) return { ...EMPTY_MUSIC_PIP, at: now }
  return {
    hasTrack: true,
    trackId: track.Id ?? undefined,
    title: trackTitle(track),
    artist: trackArtist(track),
    album: track.Album ?? undefined,
    artworkUrl: trackArt(track, ARTWORK_MAX),
    playing: s.wantPlaying && s.status !== 'paused' && s.status !== 'error',
    position: s.position,
    duration: s.duration,
    favorite,
    at: now,
  }
}

/**
 * Main-window side of the music PiP, mounted once (in the mini player): keeps the PiP window
 * fed with the player's state, runs the commands it sends back, and owns the Ctrl/Cmd+Shift+M
 * shortcut. Nothing is sent while no PiP is open.
 */
export function useMusicPipHost(): void {
  const pip = usePipState()
  const track = useCurrentTrack()
  const favorite = useIsFavorite(track?.Id ?? undefined)
  const toggleFavorite = useToggleFavorite()

  const live = useRef({ pip, favorite, trackId: track?.Id, toggleFavorite })
  live.current = { pip, favorite, trackId: track?.Id, toggleFavorite }
  const open = pip.open && pip.kind === 'music'

  // State -> PiP.
  const lastSent = useRef<MusicPipState | null>(null)
  const forcePush = useRef<(() => void) | null>(null)
  useEffect(() => {
    if (!open) {
      lastSent.current = null
      forcePush.current = null
      return
    }
    let lastSeek = useMusicPlayer.getState().seekNonce
    const push = (force: boolean) => {
      const s = useMusicPlayer.getState()
      const next = buildMusicPipState(s, live.current.favorite, Date.now())
      const prev = lastSent.current
      const seeked = s.seekNonce !== lastSeek
      lastSeek = s.seekNonce
      const stale = !prev || (next.playing && next.at - prev.at >= RESYNC_MS)
      if (force || seeked || stale || (prev && musicPipChanged(prev, next))) {
        lastSent.current = next
        sendPip(PIP_EVENTS.musicState, next)
      }
    }
    forcePush.current = () => push(true)
    push(true)
    const unsubStore = useMusicPlayer.subscribe(() => push(false))
    return () => {
      unsubStore()
      forcePush.current = null
    }
  }, [open])

  // A new PiP window asks for the state as soon as it is loaded. This listener is always on: the
  // request can arrive before this component has seen the window open.
  useEffect(() => onPip(PIP_EVENTS.ready, () => forcePush.current?.()), [])

  // The favourite flag can change without the player store changing.
  useEffect(() => {
    if (!open || !lastSent.current) return
    const next = buildMusicPipState(useMusicPlayer.getState(), favorite, Date.now())
    if (musicPipChanged(lastSent.current, next)) {
      lastSent.current = next
      sendPip(PIP_EVENTS.musicState, next)
    }
  }, [open, favorite, track?.Id])

  // Commands <- PiP.
  useEffect(() => {
    if (!open) return
    return onPip<MusicPipCommand>(PIP_EVENTS.musicCommand, (cmd) => {
      switch (cmd.type) {
        case 'toggle':
          return musicPlayer.toggle()
        case 'next':
          return musicPlayer.next()
        case 'previous':
          return musicPlayer.previous()
        case 'seek':
          return musicPlayer.seek(cmd.positionSec)
        case 'favorite': {
          const { trackId, favorite: fav, toggleFavorite: toggle } = live.current
          if (trackId) toggle.mutate({ id: trackId, value: !fav })
        }
      }
    })
  }, [open])

  // Ctrl/Cmd+Shift+M.
  useEffect(() => {
    if (!window.orphyn?.pip) return
    const onKey = (e: KeyboardEvent) => {
      if (!e.shiftKey || e.altKey || !(e.ctrlKey || e.metaKey)) return
      if (e.key.toLowerCase() !== 'm') return
      e.preventDefault()
      void togglePip('music', live.current.pip)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])
}
