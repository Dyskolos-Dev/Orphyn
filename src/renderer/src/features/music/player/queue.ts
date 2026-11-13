import type { QueueItem, QueueModel, RepeatMode, Rng, Track } from './types'

/** A previous press restarts the current track when more than this many seconds were played. */
export const PREVIOUS_RESTART_SECONDS = 3

let counter = 0
export const newUid = (): string => `q${Date.now().toString(36)}${(counter++).toString(36)}`

export const toItems = (tracks: readonly Track[]): QueueItem[] =>
  tracks.map((track) => ({ uid: newUid(), track }))

export const emptyQueue = (): QueueModel => ({
  queue: [],
  original: null,
  index: -1,
  shuffle: false,
  repeat: 'off',
})

/** Fisher-Yates, returns a new array. */
export function shuffled<T>(items: readonly T[], rng: Rng = Math.random): T[] {
  const a = items.slice()
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1))
    ;[a[i], a[j]] = [a[j] as T, a[i] as T]
  }
  return a
}

export const currentItem = (m: QueueModel): QueueItem | undefined => m.queue[m.index]

/**
 * Starts a fresh queue. With shuffle on, the chosen track (or a random one when `startIndex` is
 * undefined, as for a "Shuffle" button) comes first and the rest follows in random order.
 */
export function createQueue(
  tracks: readonly Track[],
  startIndex: number | undefined,
  opts: { shuffle: boolean; repeat: RepeatMode },
  rng: Rng = Math.random,
): QueueModel {
  const items = toItems(tracks)
  if (items.length === 0) return { ...emptyQueue(), shuffle: opts.shuffle, repeat: opts.repeat }
  if (!opts.shuffle) {
    const index = Math.min(Math.max(startIndex ?? 0, 0), items.length - 1)
    return { queue: items, original: null, index, shuffle: false, repeat: opts.repeat }
  }
  const start =
    startIndex === undefined
      ? Math.floor(rng() * items.length)
      : Math.min(Math.max(startIndex, 0), items.length - 1)
  const first = items[start] as QueueItem
  const rest = shuffled(
    items.filter((_, i) => i !== start),
    rng,
  )
  return { queue: [first, ...rest], original: items, index: 0, shuffle: true, repeat: opts.repeat }
}

/** "Play Next": inserts right after the current entry (the start of an empty queue). */
export function insertNext(m: QueueModel, tracks: readonly Track[]): QueueModel {
  const items = toItems(tracks)
  if (items.length === 0) return m
  const at = m.index + 1
  const queue = [...m.queue.slice(0, at), ...items, ...m.queue.slice(at)]
  let original = m.original
  if (original) {
    const cur = currentItem(m)
    const pos = cur ? original.findIndex((i) => i.uid === cur.uid) + 1 : 0
    original = [...original.slice(0, pos), ...items, ...original.slice(pos)]
  }
  return { ...m, queue, original, index: m.index < 0 ? 0 : m.index }
}

/** "Add to queue / Play Later": appends at the end. */
export function append(m: QueueModel, tracks: readonly Track[]): QueueModel {
  const items = toItems(tracks)
  if (items.length === 0) return m
  return {
    ...m,
    queue: [...m.queue, ...items],
    original: m.original ? [...m.original, ...items] : null,
    index: m.index < 0 ? 0 : m.index,
  }
}

export interface RemoveResult {
  model: QueueModel
  /** The removed entry was the current one: the entry now at `index` has to be loaded. */
  removedCurrent: boolean
}

export function removeAt(m: QueueModel, i: number): RemoveResult {
  const item = m.queue[i]
  if (!item) return { model: m, removedCurrent: false }
  const queue = m.queue.filter((_, k) => k !== i)
  const original = m.original ? m.original.filter((x) => x.uid !== item.uid) : null
  const removedCurrent = i === m.index
  let index = m.index
  if (queue.length === 0) index = -1
  else if (i < m.index) index = m.index - 1
  else if (removedCurrent) index = Math.min(m.index, queue.length - 1)
  return { model: { ...m, queue, original, index }, removedCurrent }
}

/** Drag reorder: moves entry `from` so that it ends at position `to`. */
export function move(m: QueueModel, from: number, to: number): QueueModel {
  if (from === to || from < 0 || to < 0 || from >= m.queue.length || to >= m.queue.length) return m
  const queue = m.queue.slice()
  const [item] = queue.splice(from, 1)
  queue.splice(to, 0, item as QueueItem)
  let index = m.index
  if (from === m.index) index = to
  else if (from < m.index && to >= m.index) index = m.index - 1
  else if (from > m.index && to <= m.index) index = m.index + 1
  return { ...m, queue, index }
}

/** Removes every entry after the current one. */
export function clearUpcoming(m: QueueModel): QueueModel {
  if (m.index < 0) return m
  const keep = m.queue.slice(0, m.index + 1)
  const uids = new Set(keep.map((i) => i.uid))
  return { ...m, queue: keep, original: m.original?.filter((i) => uids.has(i.uid)) ?? null }
}

/**
 * Turns shuffle on (played entries and the current one stay, the upcoming ones are shuffled) or
 * off (original order restored around the current entry).
 */
export function setShuffle(m: QueueModel, on: boolean, rng: Rng = Math.random): QueueModel {
  if (on === m.shuffle) return m
  if (on) {
    if (m.index < 0) return { ...m, shuffle: true, original: m.queue }
    const head = m.queue.slice(0, m.index + 1)
    const tail = shuffled(m.queue.slice(m.index + 1), rng)
    return { ...m, shuffle: true, original: m.queue, queue: [...head, ...tail] }
  }
  const cur = currentItem(m)
  const present = new Set(m.queue.map((i) => i.uid))
  const queue = (m.original ?? m.queue).filter((i) => present.has(i.uid))
  // Entries unknown to `original` are kept at the end.
  const known = new Set(queue.map((i) => i.uid))
  for (const i of m.queue) if (!known.has(i.uid)) queue.push(i)
  const index = cur ? queue.findIndex((i) => i.uid === cur.uid) : -1
  return { ...m, shuffle: false, original: null, queue, index }
}

export const cycleRepeat = (r: RepeatMode): RepeatMode =>
  r === 'off' ? 'all' : r === 'all' ? 'one' : 'off'

/**
 * Index to play after the current entry, or null when the queue is over.
 * `auto` = the track ended by itself (repeat one loops); a user "next" always moves on.
 */
export function nextIndex(m: QueueModel, reason: 'auto' | 'user'): number | null {
  if (m.queue.length === 0 || m.index < 0) return null
  if (reason === 'auto' && m.repeat === 'one') return m.index
  if (m.index + 1 < m.queue.length) return m.index + 1
  return m.repeat === 'off' ? null : 0
}

export type PreviousAction = { type: 'restart' } | { type: 'go'; index: number }

/** Apple Music rule: restart when more than 3 s were played (or on the first entry), else go back. */
export function previousAction(m: QueueModel, positionSeconds: number): PreviousAction {
  if (positionSeconds > PREVIOUS_RESTART_SECONDS || m.index <= 0) return { type: 'restart' }
  return { type: 'go', index: m.index - 1 }
}
