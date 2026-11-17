/**
 * SyncPlay ↔ music player, through the music feature's public API only (`musicPlayer` and its
 * store). The player is not changed: what the person does with it (play, pause, seek, next, a new
 * queue…) is noticed on the store and turned into group requests; what the group commands is
 * applied with the same API, flagged so it is not mistaken for the person.
 */
import { musicPlayer, useMusicPlayer } from '@renderer/features/music/player/api'
import type { PlayerState } from '@renderer/features/music/player/store'
import { syncPlay as sp, type BaseItemDto } from '@renderer/jellyfin'
import { queueMove } from './logic'
import { useSyncPlay, type MusicBridge, type MusicIntents } from './manager'
import type { SyncPlayer } from './types'

let applying = 0
function quietly(run: () => void): void {
  applying++
  try {
    run()
  } finally {
    applying--
  }
}

const idsOf = (s: PlayerState): string[] => s.queue.map((q) => sp.normalizeGuid(q.track.Id))
const sameIds = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && a.every((id, i) => id === b[i])

/** `next` is `prev` with entries added at the end: the extra ids, else null. */
export function appended(prev: readonly string[], next: readonly string[]): string[] | null {
  if (next.length <= prev.length || !prev.every((id, i) => id === next[i])) return null
  return next.slice(prev.length)
}

/** `next` is `prev` with entries inserted right after `index` ("Play Next"): those ids, else null. */
export function insertedAfter(
  prev: readonly string[],
  next: readonly string[],
  index: number,
): string[] | null {
  const added = next.length - prev.length
  if (added <= 0 || index < 0) return null
  const head = prev.slice(0, index + 1)
  const tail = prev.slice(index + 1)
  if (!head.every((id, i) => id === next[i])) return null
  if (!tail.every((id, i) => id === next[index + 1 + added + i])) return null
  return next.slice(index + 1, index + 1 + added)
}

function outputPosition(s: PlayerState): number {
  // The store's position moves with timeupdate (4 per second); the output knows it exactly.
  if (s.status !== 'playing') return s.position
  try {
    return musicPlayer.getOutput().position
  } catch {
    return s.position
  }
}

const player: SyncPlayer = {
  kind: 'audio',
  itemId() {
    const s = useMusicPlayer.getState()
    const id = s.queue[s.index]?.track.Id
    return id ? sp.normalizeGuid(id) : null
  },
  position: () => outputPosition(useMusicPlayer.getState()),
  isPlaying: () => useMusicPlayer.getState().status === 'playing',
  isReady() {
    const s = useMusicPlayer.getState()
    return (s.status === 'paused' || s.status === 'playing') && s.duration > 0
  },
  play: () => quietly(() => musicPlayer.resume()),
  pause: () => quietly(() => musicPlayer.pause()),
  seek: (seconds) => quietly(() => musicPlayer.seek(seconds)),
  subscribe: (listener) => useMusicPlayer.subscribe(listener),
}

export interface QueueIntents extends MusicIntents {
  queue?(itemIds: string[], mode: 'Queue' | 'QueueNext'): void
}

/** Turns a person's change of the music player into a group request (pure, for tests). */
export function musicIntent(
  s: PlayerState,
  prev: PlayerState,
  followingAudio: boolean,
  intents: QueueIntents,
): void {
  const prevIds = idsOf(prev)
  const ids = idsOf(s)
  if (s.queue !== prev.queue && !sameIds(prevIds, ids)) {
    if (ids.length === 0 || s.index < 0) return
    const tail = followingAudio && s.index === prev.index ? appended(prevIds, ids) : null
    if (tail && intents.queue) return intents.queue(tail, 'Queue')
    const next =
      followingAudio && s.index === prev.index ? insertedAfter(prevIds, ids, prev.index) : null
    if (next && intents.queue) return intents.queue(next, 'QueueNext')
    return intents.newQueue(ids, s.index, s.position)
  }
  if (!followingAudio) return
  if (s.index !== prev.index && s.index >= 0) {
    const move = queueMove(prev.index, s.index, ids.length)
    const current = prevIds[prev.index] ?? null
    if (move === 'next') intents.next(current)
    else if (move === 'previous') intents.previous(current)
    else if (move === 'jump') intents.jump(ids[s.index] ?? '')
    return
  }
  if (s.seekNonce !== prev.seekNonce) return intents.seek(s.position)
  if (s.wantPlaying !== prev.wantPlaying) {
    if (s.wantPlaying) {
      // Wait for the group: it starts everyone at the same moment.
      quietly(() => musicPlayer.pause())
      intents.play()
    } else intents.pause()
  }
}

let unsubscribe: (() => void) | null = null

export function createMusicBridge(
  queueItems: (ids: string[], mode: 'Queue' | 'QueueNext') => void,
): MusicBridge {
  return {
    player,
    localQueue() {
      const s = useMusicPlayer.getState()
      return { ids: idsOf(s), index: s.index, playing: s.status === 'playing' }
    },
    load(tracks: BaseItemDto[], index: number) {
      quietly(() => {
        musicPlayer.play(tracks, index, { shuffle: false })
        musicPlayer.pause()
      })
    },
    jump(index: number) {
      quietly(() => {
        useMusicPlayer.getState().jumpTo(index)
        musicPlayer.pause()
      })
    },
    extend(tracks: BaseItemDto[]) {
      const s = useMusicPlayer.getState()
      const local = idsOf(s)
      const group = tracks.map((t) => sp.normalizeGuid(t.Id))
      const byId = new Map(tracks.map((t) => [sp.normalizeGuid(t.Id), t]))
      const pick = (ids: string[]) =>
        ids.map((id) => byId.get(id)).filter((t): t is BaseItemDto => !!t)
      const tail = appended(local, group)
      if (tail) return quietly(() => musicPlayer.addToQueue(pick(tail)))
      const next = insertedAfter(local, group, s.index)
      if (next) quietly(() => musicPlayer.playNext(pick(next)))
    },
    start(intents: MusicIntents) {
      unsubscribe?.()
      const all: QueueIntents = { ...intents, queue: queueItems }
      unsubscribe = useMusicPlayer.subscribe((s, prev) => {
        if (applying) return
        musicIntent(s, prev, useSyncPlay.getState().currentKind === 'audio', all)
      })
    },
    stop() {
      unsubscribe?.()
      unsubscribe = null
    },
  }
}
