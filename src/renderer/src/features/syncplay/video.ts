/**
 * SyncPlay ↔ video player, through the video feature's `PlaybackController` (the `<video>`
 * element it exposes is used for the speed nudges only). The player calls `useSyncPlayVideo`;
 * the controller it gets back sends play / pause / seek to the group while in one.
 */
import { useEffect, useMemo, useRef } from 'react'
import { syncPlay as sp } from '@renderer/jellyfin'
import type {
  PlaybackController,
  VideoPlaybackController,
} from '@renderer/features/video/controller'
import { syncPlayManager, useSyncPlay } from './manager'
import type { SyncPlayer } from './types'

const HAVE_FUTURE_DATA = 3
const ELEMENT_EVENTS = [
  'canplay',
  'canplaythrough',
  'seeked',
  'seeking',
  'waiting',
  'playing',
  'pause',
] as const

export function videoSyncPlayer(controller: VideoPlaybackController): SyncPlayer {
  const el = () => controller.element
  return {
    kind: 'video',
    itemId() {
      const id = controller.getSnapshot().itemId
      return id ? sp.normalizeGuid(id) : null
    },
    position: () => el()?.currentTime ?? controller.getSnapshot().positionSeconds,
    isPlaying() {
      const v = el()
      return Boolean(v && !v.paused && !v.ended)
    },
    isReady() {
      const v = el()
      const status = controller.getSnapshot().status
      return Boolean(
        v &&
        status !== 'error' &&
        status !== 'idle' &&
        !v.seeking &&
        v.readyState >= HAVE_FUTURE_DATA,
      )
    },
    play: () => controller.resume(),
    pause: () => controller.pause(),
    seek: (seconds) => controller.seek(seconds),
    setRate(rate) {
      const v = el()
      if (v && v.playbackRate !== rate) v.playbackRate = rate
    },
    subscribe(listener) {
      const off = controller.subscribe(listener)
      const v = el()
      ELEMENT_EVENTS.forEach((name) => v?.addEventListener(name, listener))
      return () => {
        off()
        ELEMENT_EVENTS.forEach((name) => v?.removeEventListener(name, listener))
      }
    },
  }
}

export interface SyncPlayVideo {
  /** In a group (and not a trailer). */
  inGroup: boolean
  /** Use it for every play / pause / seek of the person (buttons, keys, OS media keys). */
  controller: PlaybackController
  togglePlay(): void
}

/**
 * Hooks the open video into the group: registers the player while in a group, shares a film the
 * person opens, and routes their play / pause / seek to the group. Trailers stay out of it.
 */
export function useSyncPlayVideo(
  controller: VideoPlaybackController,
  itemId: string,
  trailer: boolean,
): SyncPlayVideo {
  const phase = useSyncPlay((s) => s.phase)
  const inGroup = !trailer && (phase === 'joined' || phase === 'reconnecting')
  // Registered from 'connecting' on, so a room created from here finds the film to share.
  const attach = !trailer && phase !== 'off'

  useEffect(() => {
    if (!attach) return
    return syncPlayManager.attachVideo(videoSyncPlayer(controller))
  }, [attach, controller])

  // An item opened here while in a group becomes the group's. Joining while a film is open shares
  // it through the manager instead (create) or replaces it (join), so that is not a change.
  const lastItem = useRef<string | null>(null)
  useEffect(() => {
    if (!inGroup) {
      lastItem.current = itemId
      return
    }
    if (!itemId || lastItem.current === itemId) return
    lastItem.current = itemId
    syncPlayManager.videoItemStarted(itemId, controller.getSnapshot().positionSeconds)
  }, [inGroup, itemId, controller])

  return useMemo(() => {
    const group: PlaybackController = {
      getSnapshot: controller.getSnapshot,
      subscribe: controller.subscribe,
      play: (id, start, options) => controller.play(id, start, options),
      pause: () => (inGroup ? syncPlayManager.requestPause() : controller.pause()),
      resume: () => (inGroup ? syncPlayManager.requestPlay() : controller.resume()),
      seek: (s) => (inGroup ? syncPlayManager.requestSeek(s) : controller.seek(s)),
      stop: () => controller.stop(),
    }
    const togglePlay = () => {
      if (inGroup) {
        const v = controller.element
        if (v && !v.paused) syncPlayManager.requestPause()
        else syncPlayManager.requestPlay()
        return
      }
      const status = controller.getSnapshot().status
      if (status === 'playing' || status === 'loading') controller.pause()
      else controller.resume()
    }
    return { inGroup, controller: group, togglePlay }
  }, [controller, inGroup])
}
