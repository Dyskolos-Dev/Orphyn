/**
 * Radios: an endless queue seeded by a song, an album or an artist. The queue is refilled in the
 * background before it runs out, and the radio ends by itself when the user plays something else.
 */
import { create } from 'zustand'
import { diversify } from './generate'
import type { Track } from './types'

/** Songs left after the current one under which the queue is refilled. */
export const REFILL_THRESHOLD = 8
/** Failed (empty) refills in a row after which the radio gives up. */
const MAX_EMPTY_REFILLS = 3
const BATCH = 25

export interface RadioSeed {
  id: string
  name: string
  kind: 'song' | 'album' | 'artist'
}

export interface RadioState {
  active: boolean
  seed: RadioSeed | null
}

export const useRadio = create<RadioState>(() => ({ active: false, seed: null }))

/** What the controller needs from the player (the real one is the player store). */
export interface RadioPlayer {
  getState(): { queue: readonly { track: Track }[]; index: number }
  subscribe(listener: () => void): () => void
  addToQueue(tracks: readonly Track[]): void
}

export interface RadioDeps {
  player: RadioPlayer
  /** More songs for the seed; `currentId` is the song playing now. */
  fetchMore(seedId: string, currentId?: string): Promise<Track[]>
  threshold?: number
}

/**
 * The songs of `candidates` a radio may append: none heard or queued already, at most 3 per
 * artist, and the artist of the last queued song not first (so two songs of one artist never meet).
 */
export function pickRefill(
  candidates: readonly Track[],
  avoid: ReadonlySet<string>,
  lastArtist: string | undefined,
  limit = BATCH,
): Track[] {
  const fresh = diversify(candidates, { limit: limit * 2, exclude: avoid })
  const first = fresh.findIndex(
    (t) => !lastArtist || (t.ArtistItems?.[0]?.Id ?? t.Artists?.[0] ?? '') !== lastArtist,
  )
  if (first > 0) fresh.unshift(fresh.splice(first, 1)[0] as Track)
  return fresh.slice(0, limit)
}

export interface RadioController {
  /** Starts a radio whose first songs are `first` (already queued by the caller). */
  start(seed: RadioSeed, first: readonly Track[]): void
  stop(): void
  /** Looks at the queue and refills it when it is running low. Called on every player change. */
  check(): void
}

export function createRadioController(
  deps: RadioDeps,
  publish: (s: RadioState) => void = (s) => useRadio.setState(s),
): RadioController {
  const threshold = deps.threshold ?? REFILL_THRESHOLD
  let seed: RadioSeed | null = null
  /** Ids the radio put in the queue (also its "already heard" memory). */
  const known = new Set<string>()
  let busy = false
  let empty = 0
  let unsubscribe: (() => void) | null = null
  let lastQueue: unknown = null
  let lastIndex = -2

  const stop = () => {
    unsubscribe?.()
    unsubscribe = null
    seed = null
    known.clear()
    busy = false
    empty = 0
    publish({ active: false, seed: null })
  }

  const refill = async () => {
    if (!seed || busy) return
    busy = true
    const mine = seed
    try {
      const { queue, index } = deps.player.getState()
      const more = await deps.fetchMore(mine.id, queue[index]?.track.Id)
      if (seed !== mine) return
      const state = deps.player.getState()
      const avoid = new Set<string>(known)
      for (const q of state.queue) if (q.track.Id) avoid.add(q.track.Id)
      const last = state.queue[state.queue.length - 1]?.track
      const picked = pickRefill(more, avoid, last?.ArtistItems?.[0]?.Id ?? last?.Artists?.[0])
      if (picked.length === 0) {
        empty += 1
        if (empty >= MAX_EMPTY_REFILLS) return stop()
        return
      }
      empty = 0
      for (const t of picked) if (t.Id) known.add(t.Id)
      deps.player.addToQueue(picked)
    } catch {
      empty += 1
      if (empty >= MAX_EMPTY_REFILLS) return stop()
    } finally {
      busy = false
    }
    // The queue may still be short (small batch): look again.
    if (seed === mine && empty === 0) controller.check()
  }

  const controller: RadioController = {
    start(next, first) {
      stop()
      seed = next
      for (const t of first) if (t.Id) known.add(t.Id)
      publish({ active: true, seed: next })
      unsubscribe = deps.player.subscribe(() => controller.check())
      controller.check()
    },
    stop,
    check() {
      if (!seed) return
      const { queue, index } = deps.player.getState()
      // The player reports progress many times a second: only look when the queue moved.
      if (queue === lastQueue && index === lastIndex && !busy) return
      lastQueue = queue
      lastIndex = index
      const current = queue[index]?.track.Id
      // Queue emptied, or the user started something else: the radio is over.
      if (queue.length === 0 || (current && !known.has(current))) return stop()
      if (queue.length - 1 - index < threshold && empty < MAX_EMPTY_REFILLS) void refill()
    },
  }
  return controller
}
