import { leanTrack } from './track'
import type { QueueItem, QueueModel, RepeatMode } from './types'

export const STORAGE_KEY = 'orphyn.music.player'
/** Queues longer than this are cut around the current entry when saved. */
export const MAX_PERSISTED = 300
const VERSION = 1

export interface PersistedPlayer {
  version: number
  queue: QueueItem[]
  /** Original order while shuffled (uids only: the entries themselves are in `queue`). */
  originalUids: string[] | null
  index: number
  position: number
  shuffle: boolean
  repeat: RepeatMode
  volume: number
  muted: boolean
}

export interface PlayerSnapshot extends QueueModel {
  position: number
  volume: number
  muted: boolean
}

export function serializePlayer(s: PlayerSnapshot): PersistedPlayer {
  let { queue, index } = s
  if (queue.length > MAX_PERSISTED) {
    const start = Math.max(0, Math.min(index - 20, queue.length - MAX_PERSISTED))
    queue = queue.slice(start, start + MAX_PERSISTED)
    index -= start
  }
  const kept = new Set(queue.map((i) => i.uid))
  return {
    version: VERSION,
    queue: queue.map((i) => ({ uid: i.uid, track: leanTrack(i.track) })),
    originalUids: s.original ? s.original.filter((i) => kept.has(i.uid)).map((i) => i.uid) : null,
    index,
    position: Math.max(0, s.position),
    shuffle: s.shuffle,
    repeat: s.repeat,
    volume: s.volume,
    muted: s.muted,
  }
}

const isRepeat = (v: unknown): v is RepeatMode => v === 'off' || v === 'all' || v === 'one'

/** Validates untrusted stored JSON; returns null when it is unusable. */
export function deserializePlayer(raw: unknown): PlayerSnapshot | null {
  if (!raw || typeof raw !== 'object') return null
  const p = raw as Partial<PersistedPlayer>
  if (p.version !== VERSION || !Array.isArray(p.queue)) return null
  const queue = p.queue.filter(
    (i): i is QueueItem => !!i && typeof i.uid === 'string' && !!i.track?.Id,
  )
  if (queue.length === 0) return null
  const byUid = new Map(queue.map((i) => [i.uid, i]))
  const original =
    p.shuffle && Array.isArray(p.originalUids)
      ? p.originalUids.map((u) => byUid.get(u)).filter((i): i is QueueItem => !!i)
      : null
  const index = typeof p.index === 'number' ? Math.min(Math.max(p.index, 0), queue.length - 1) : 0
  return {
    queue,
    original: original && original.length > 0 ? original : p.shuffle ? queue : null,
    index,
    position: typeof p.position === 'number' && p.position >= 0 ? p.position : 0,
    shuffle: !!p.shuffle,
    repeat: isRepeat(p.repeat) ? p.repeat : 'off',
    volume: typeof p.volume === 'number' ? Math.min(1, Math.max(0, p.volume)) : 1,
    muted: !!p.muted,
  }
}

export function savePlayer(s: PlayerSnapshot, storage: Storage = localStorage): void {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(serializePlayer(s)))
  } catch {
    /* quota or private mode: persistence is best effort */
  }
}

export function loadPlayer(storage: Storage = localStorage): PlayerSnapshot | null {
  try {
    const raw = storage.getItem(STORAGE_KEY)
    return raw ? deserializePlayer(JSON.parse(raw)) : null
  } catch {
    return null
  }
}

export function clearPlayer(storage: Storage = localStorage): void {
  try {
    storage.removeItem(STORAGE_KEY)
  } catch {
    /* ignore */
  }
}
