import type { BaseItemDto } from '@renderer/jellyfin'

/** A playable track: a Jellyfin `Audio` item. */
export type Track = BaseItemDto

/** One entry of the queue. `uid` is unique per entry so the same track can be queued twice. */
export interface QueueItem {
  uid: string
  track: Track
}

export type RepeatMode = 'off' | 'all' | 'one'

export type PlayerStatus = 'idle' | 'loading' | 'playing' | 'paused' | 'error'

/** Pure queue model shared by the store and its tests. */
export interface QueueModel {
  /** Playback order (already shuffled when `shuffle` is on). */
  queue: QueueItem[]
  /** Original order, kept only while shuffle is on, to restore it when shuffle is turned off. */
  original: QueueItem[] | null
  /** Index of the current entry in `queue`; -1 when the queue is empty. */
  index: number
  shuffle: boolean
  repeat: RepeatMode
}

/** Anything that can produce a number in [0, 1), injectable for deterministic tests. */
export type Rng = () => number
