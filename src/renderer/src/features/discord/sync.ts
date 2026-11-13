import { subscribeActiveController, getActivePlaybackController } from '@renderer/features/video'
import { useMusicPlayer } from '@renderer/features/music'
import * as ep from '@renderer/jellyfin/endpoints'
import { useSession, type BaseItemDto } from '@renderer/jellyfin'
import type { DiscordPresence } from '@shared/ipc/discord'
import { musicPresence, shouldSend, videoPresence, type Sent } from './presence'

/**
 * Watches the music player store and the active video controller, and sends one normalized
 * presence to the main process when something worth showing changes. Video wins while it plays.
 * Returns a function that stops it (and clears the presence).
 */
export function startDiscordSync(
  send: (presence: DiscordPresence | null) => void = (p) => {
    void window.lumen?.discord?.setPresence(p)
  },
): () => void {
  let sent: Sent | null = null
  let videoItem: BaseItemDto | undefined
  let fetching: string | null = null
  let unsubVideo: (() => void) | null = null
  let stopped = false

  const evaluate = (): void => {
    if (stopped) return
    const now = Date.now()
    const controller = getActivePlaybackController()
    const video = controller ? videoPresence(controller.getSnapshot(), videoItem) : null
    let next = video
    if (!next) {
      const s = useMusicPlayer.getState()
      next = musicPresence({
        track: s.queue[s.index]?.track,
        status: s.status,
        position: s.position,
        duration: s.duration,
      })
    }
    // A video that is loading its metadata must not let a paused/old music presence flash.
    if (controller && !video) {
      const snap = controller.getSnapshot()
      if (snap.itemId && snap.status !== 'idle' && snap.status !== 'ended') {
        void loadVideoItem(snap.itemId)
      }
    }
    if (!shouldSend(sent, next, now)) return
    sent = { presence: next, at: now }
    send(next)
  }

  const loadVideoItem = async (id: string): Promise<void> => {
    if (fetching === id || videoItem?.Id === id) return
    fetching = id
    try {
      const { api, account } = useSession.getState()
      if (!api || !account) return
      const item = await ep.fetchItem(api, account.userId, id)
      if (fetching === id) videoItem = item
    } catch {
      // No metadata: the video simply shows no presence.
    } finally {
      if (fetching === id) fetching = null
      evaluate()
    }
  }

  const attachVideo = (): void => {
    unsubVideo?.()
    unsubVideo = null
    const controller = getActivePlaybackController()
    if (controller) unsubVideo = controller.subscribe(evaluate)
    evaluate()
  }

  const unsubMusic = useMusicPlayer.subscribe(evaluate)
  const unsubActive = subscribeActiveController(attachVideo)
  attachVideo()

  return () => {
    stopped = true
    unsubMusic()
    unsubActive()
    unsubVideo?.()
    send(null)
  }
}
