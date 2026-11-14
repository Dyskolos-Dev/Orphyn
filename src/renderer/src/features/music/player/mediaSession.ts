import { claimMediaControls, nativeMediaControls } from '@renderer/bridge/mediaControls'
import type { MediaControlAction } from '@shared/ipc/mediaControls'
import { usePlayerStore, type PlayerState } from './store'
import { currentItem } from './queue'
import { trackArt, trackArtist, trackTitle } from './track'

const SKIP_SECONDS = 10
export const ARTWORK_SIZES = [96, 128, 256, 384, 512] as const

type SessionLike = Pick<MediaSession, 'setActionHandler' | 'setPositionState'> & {
  metadata: MediaMetadata | null
  playbackState: MediaSessionPlaybackState
}

function metadataFor(s: PlayerState): MediaMetadataInit | null {
  const track = currentItem(s)?.track
  if (!track) return null
  const artwork = ARTWORK_SIZES.flatMap((size) => {
    const src = trackArt(track, size)
    return src ? [{ src, sizes: `${size}x${size}`, type: 'image/jpeg' }] : []
  })
  return {
    title: trackTitle(track),
    artist: trackArtist(track),
    album: track.Album ?? '',
    artwork,
  }
}

/**
 * Publishes the current track and transport controls to `navigator.mediaSession`. Chromium maps it
 * to Windows SMTC (media keys + flyout), macOS Now Playing and Linux MPRIS.
 * Returns an uninstall function. `session` is injectable for tests.
 */
function installWebMediaSession(
  session: SessionLike | undefined = typeof navigator !== 'undefined' && 'mediaSession' in navigator
    ? navigator.mediaSession
    : undefined,
  MetadataCtor: typeof MediaMetadata | undefined = typeof MediaMetadata !== 'undefined'
    ? MediaMetadata
    : undefined,
): () => void {
  if (!session || !MetadataCtor) return () => {}
  const store = usePlayerStore
  const handlers: [MediaSessionAction, MediaSessionActionHandler | null][] = [
    ['play', () => store.getState().resume()],
    ['pause', () => store.getState().pause()],
    ['stop', () => store.getState().pause()],
    ['previoustrack', () => store.getState().previous()],
    ['nexttrack', () => store.getState().next()],
    ['seekto', (d) => typeof d.seekTime === 'number' && store.getState().seek(d.seekTime)],
    [
      'seekbackward',
      (d) => store.getState().seek(store.getState().position - (d.seekOffset ?? SKIP_SECONDS)),
    ],
    [
      'seekforward',
      (d) => store.getState().seek(store.getState().position + (d.seekOffset ?? SKIP_SECONDS)),
    ],
  ]
  for (const [action, handler] of handlers) {
    try {
      session.setActionHandler(action, handler)
    } catch {
      /* action not supported on this platform */
    }
  }

  const publishPosition = (s: PlayerState) => {
    if (!(s.duration > 0)) return
    try {
      session.setPositionState({
        duration: s.duration,
        position: Math.min(Math.max(0, s.position), s.duration),
        playbackRate: 1,
      })
    } catch {
      /* invalid state while loading */
    }
  }

  const apply = (s: PlayerState, prev?: PlayerState) => {
    const changedTrack = !prev || currentItem(s)?.uid !== currentItem(prev)?.uid
    if (changedTrack) {
      const meta = metadataFor(s)
      session.metadata = meta ? new MetadataCtor(meta) : null
    }
    session.playbackState = !currentItem(s) ? 'none' : s.status === 'playing' ? 'playing' : 'paused'
    // Position: on seeks, track changes, duration discovery and play/pause (not every tick).
    if (
      changedTrack ||
      !prev ||
      s.seekNonce !== prev.seekNonce ||
      s.status !== prev.status ||
      (prev.duration === 0 && s.duration > 0)
    ) {
      publishPosition(s)
    }
  }

  apply(store.getState())
  const unsub = store.subscribe(apply)
  return () => {
    unsub()
    for (const [action] of handlers) {
      try {
        session.setActionHandler(action, null)
      } catch {
        /* ignore */
      }
    }
    session.metadata = null
  }
}

/** Seeks / resume / pause / skips: what an OS media button asks of the music player. */
export function runMusicAction(action: MediaControlAction): void {
  const s = usePlayerStore.getState()
  switch (action.type) {
    case 'play':
      return s.resume()
    case 'pause':
    case 'stop':
      return s.pause()
    case 'toggle':
      return s.toggle()
    case 'next':
      return s.next()
    case 'previous':
      return s.previous()
    case 'seek':
      return s.seek(action.positionSec)
    case 'seekBy':
      return s.seek(s.position + action.offsetSec)
  }
}

/**
 * Native OS controls (Tauri shell): metadata, playback and position go to Rust, the OS buttons
 * come back as actions. The music player claims the session while nothing else (a video) does.
 */
function installNativeMediaControls(): () => void {
  const store = usePlayerStore
  const holder: { claim?: ReturnType<typeof claimMediaControls> } = {}

  const publishMetadata = (s: PlayerState) => {
    const track = currentItem(s)?.track
    if (!track) return holder.claim?.setPlayback({ status: 'stopped' })
    holder.claim?.setMetadata({
      title: trackTitle(track),
      artist: trackArtist(track),
      album: track.Album ?? undefined,
      artworkUrl: trackArt(track, 512),
      durationSec: s.duration > 0 ? s.duration : undefined,
    })
  }
  const publishPlayback = (s: PlayerState) => {
    if (!currentItem(s)) return
    holder.claim?.setPlayback({
      status: s.status === 'playing' ? 'playing' : 'paused',
      positionSec: Math.max(0, s.position),
    })
  }
  const refresh = () => {
    const s = store.getState()
    if (!currentItem(s))
      return void nativeMediaControls()
        ?.clear()
        .catch(() => undefined)
    publishMetadata(s)
    publishPlayback(s)
  }

  holder.claim = claimMediaControls({ onAction: runMusicAction, refresh })
  refresh()
  const unsub = store.subscribe((s, prev) => {
    const changedTrack = currentItem(s)?.uid !== currentItem(prev)?.uid
    // The duration arrives after the track: republish the metadata with it.
    if (changedTrack || (prev.duration === 0 && s.duration > 0)) publishMetadata(s)
    if (
      changedTrack ||
      s.seekNonce !== prev.seekNonce ||
      s.status !== prev.status ||
      (prev.duration === 0 && s.duration > 0)
    ) {
      publishPlayback(s)
    }
  })
  return () => {
    unsub()
    holder.claim?.release()
  }
}

/**
 * Publishes the current track and transport controls to the OS: through the native controls of
 * the Tauri shell when there are any (a single entry in the OS panel, shared with the video
 * player), otherwise through `navigator.mediaSession` (a plain browser maps it to its own OS
 * integration). Returns an uninstall function.
 */
export function installMediaSession(
  ...args: Parameters<typeof installWebMediaSession>
): () => void {
  if (args.length === 0 && nativeMediaControls()) return installNativeMediaControls()
  return installWebMediaSession(...args)
}
