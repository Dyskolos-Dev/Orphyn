import { useEffect, useRef } from 'react'
import { claimMediaControls, nativeMediaControls } from '@renderer/bridge/mediaControls'
import type { MediaControlAction } from '@shared/ipc/mediaControls'
import type { PlaybackSnapshot, VideoPlaybackController } from '../controller'

const SKIP_SECONDS = 10

interface Options {
  controller: VideoPlaybackController
  title: string
  artist?: string
  album?: string
  artworkUrl?: string
  snapshot: PlaybackSnapshot
  /** Next episode, when there is one. */
  playNext?: () => void
}

/** What an OS media button asks of the video player. */
export function runVideoAction(
  action: MediaControlAction,
  controller: VideoPlaybackController,
  playNext?: () => void,
): void {
  const snap = controller.getSnapshot()
  switch (action.type) {
    case 'play':
      return controller.resume()
    case 'pause':
    case 'stop':
      return controller.pause()
    case 'toggle':
      return snap.status === 'playing' || snap.status === 'loading'
        ? controller.pause()
        : controller.resume()
    case 'next':
      return playNext?.()
    case 'previous':
      return controller.seek(0)
    case 'seek':
      return controller.seek(action.positionSec)
    case 'seekBy':
      return controller.seek(snap.positionSeconds + (action.offsetSec || SKIP_SECONDS))
  }
}

/**
 * Publishes the open video to the native OS media controls (Tauri shell) and handles their
 * buttons. The video claims the session while it is open, then hands it back to the music
 * player. Returns false when the shell has no native controls: the caller then keeps using
 * `navigator.mediaSession`, so there is never a second entry in the OS panel.
 */
export function useVideoMediaControls(options: Options): boolean {
  const native = Boolean(nativeMediaControls())
  const latest = useRef(options)
  latest.current = options
  const claimRef = useRef<ReturnType<typeof claimMediaControls>>(undefined)
  const { title, artist, album, artworkUrl } = options

  // Claim for as long as the player is mounted.
  useEffect(() => {
    if (!native) return
    const publish = () => {
      const o = latest.current
      if (!o.title) return
      claimRef.current?.setMetadata({
        title: o.title,
        artist: o.artist,
        album: o.album,
        artworkUrl: o.artworkUrl,
        durationSec: o.snapshot.durationSeconds || undefined,
      })
      claimRef.current?.setPlayback({
        status: o.snapshot.status === 'playing' ? 'playing' : 'paused',
        positionSec: o.snapshot.positionSeconds,
      })
    }
    const claim = claimMediaControls({
      onAction: (a) => runVideoAction(a, latest.current.controller, latest.current.playNext),
      refresh: publish,
    })
    claimRef.current = claim
    return () => {
      claimRef.current = undefined
      claim?.release()
    }
  }, [native])

  // Metadata: when the item (title, artwork) or its duration changes.
  const duration = options.snapshot.durationSeconds
  useEffect(() => {
    if (!title) return
    claimRef.current?.setMetadata({
      title,
      artist,
      album,
      artworkUrl,
      durationSec: duration || undefined,
    })
  }, [title, artist, album, artworkUrl, duration])

  // Playback: on status changes (play / pause / seek publish the position, not every tick).
  const status = options.snapshot.status
  useEffect(() => {
    claimRef.current?.setPlayback({
      status: status === 'playing' ? 'playing' : 'paused',
      positionSec: latest.current.snapshot.positionSeconds,
    })
  }, [status])

  return native
}
