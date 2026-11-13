import { create } from 'zustand'
import {
  append,
  clearUpcoming,
  createQueue,
  currentItem,
  cycleRepeat,
  insertNext,
  move,
  nextIndex,
  previousAction,
  removeAt,
  setShuffle,
} from './queue'
import type { PlayerStatus, QueueItem, QueueModel, RepeatMode, Track } from './types'

/**
 * Player state. Actions only change state (queue, intent); the engine (`engine.ts`) watches it and
 * drives the audio output. `loadId` changes whenever the current entry must be (re)loaded.
 */
export interface PlayerState extends QueueModel {
  status: PlayerStatus
  position: number
  duration: number
  volume: number
  muted: boolean
  /** The user wants audio (play pressed / autoplay) as opposed to paused. */
  wantPlaying: boolean
  /** Bumped when the current entry has to be loaded into the output. */
  loadId: number
  /** Start playing when that load is done. */
  autoplay: boolean
  /** Position to start at for that load (restored sessions). */
  startAt: number
  /** Bumped on every seek request; `position` already holds the target. */
  seekNonce: number

  play(tracks: readonly Track[], startIndex?: number, opts?: { shuffle?: boolean }): void
  playNext(tracks: readonly Track[]): void
  addToQueue(tracks: readonly Track[]): void
  removeAt(index: number): void
  moveItem(from: number, to: number): void
  clearUpcoming(): void
  jumpTo(index: number): void
  next(): void
  previous(): void
  pause(): void
  resume(): void
  toggle(): void
  seek(seconds: number): void
  setVolume(volume: number): void
  toggleMute(): void
  toggleShuffle(): void
  cycleRepeat(): void
  setRepeat(repeat: RepeatMode): void
  /** Empties the queue and stops. */
  stop(): void

  // --- called by the engine
  setProgress(position: number, duration: number): void
  setStatus(status: PlayerStatus): void
  /** The current track ended by itself. */
  handleEnded(): void
  /** The current track cannot be played: move on, or stop at the end of the queue. */
  skipUnplayable(): boolean
  /** Loads a saved session without playing it. */
  restore(s: QueueModel & { position: number; volume: number; muted: boolean }): void
}

const model = (s: PlayerState): QueueModel => ({
  queue: s.queue,
  original: s.original,
  index: s.index,
  shuffle: s.shuffle,
  repeat: s.repeat,
})

export const initialPlayerState = {
  queue: [] as QueueItem[],
  original: null as QueueItem[] | null,
  index: -1,
  shuffle: false,
  repeat: 'off' as RepeatMode,
  status: 'idle' as PlayerStatus,
  position: 0,
  duration: 0,
  volume: 1,
  muted: false,
  wantPlaying: false,
  loadId: 0,
  autoplay: false,
  startAt: 0,
  seekNonce: 0,
}

export const usePlayerStore = create<PlayerState>()((set, get) => {
  /** Switches to the entry at `index` and loads it. */
  const goTo = (m: QueueModel, index: number, autoplay = true) =>
    set((s) => ({
      ...m,
      index,
      position: 0,
      duration: 0,
      startAt: 0,
      autoplay,
      wantPlaying: autoplay,
      status: 'loading',
      loadId: s.loadId + 1,
    }))

  return {
    ...initialPlayerState,

    play(tracks, startIndex, opts) {
      const s = get()
      const m = createQueue(tracks, startIndex, {
        shuffle: opts?.shuffle ?? s.shuffle,
        repeat: s.repeat,
      })
      if (m.index < 0) return
      goTo(m, m.index)
    },

    playNext(tracks) {
      const s = get()
      const empty = s.queue.length === 0
      const m = insertNext(model(s), tracks)
      if (empty && m.index >= 0) goTo(m, m.index)
      else set(m)
    },

    addToQueue(tracks) {
      const s = get()
      const empty = s.queue.length === 0
      const m = append(model(s), tracks)
      if (empty && m.index >= 0) goTo(m, m.index)
      else set(m)
    },

    removeAt(i) {
      const s = get()
      const { model: m, removedCurrent } = removeAt(model(s), i)
      if (!removedCurrent) return set(m)
      if (m.index < 0) {
        set({
          ...m,
          status: 'idle',
          wantPlaying: false,
          position: 0,
          duration: 0,
          loadId: s.loadId + 1,
        })
      } else {
        goTo(m, m.index, s.wantPlaying)
      }
    },

    moveItem: (from, to) => set((s) => move(model(s), from, to)),
    clearUpcoming: () => set((s) => clearUpcoming(model(s))),

    jumpTo(i) {
      const s = get()
      if (i < 0 || i >= s.queue.length) return
      goTo(model(s), i)
    },

    next() {
      const s = get()
      const i = nextIndex(model(s), 'user')
      if (i !== null) goTo(model(s), i)
    },

    previous() {
      const s = get()
      const action = previousAction(model(s), s.position)
      if (s.index < 0) return
      if (action.type === 'restart') get().seek(0)
      else goTo(model(s), action.index)
    },

    pause: () => set({ wantPlaying: false }),
    resume: () => set((s) => (s.queue.length ? { wantPlaying: true } : {})),
    toggle() {
      if (get().wantPlaying) get().pause()
      else get().resume()
    },

    seek(seconds) {
      const s = get()
      if (s.index < 0) return
      const max = s.duration > 0 ? s.duration : Number.POSITIVE_INFINITY
      set({ position: Math.min(Math.max(0, seconds), max), seekNonce: s.seekNonce + 1 })
    },

    setVolume: (v) => set({ volume: Math.min(1, Math.max(0, v)), muted: false }),
    toggleMute: () => set((s) => ({ muted: !s.muted })),
    toggleShuffle: () => set((s) => setShuffle(model(s), !s.shuffle)),
    cycleRepeat: () => set((s) => ({ repeat: cycleRepeat(s.repeat) })),
    setRepeat: (repeat) => set({ repeat }),

    stop: () =>
      set((s) => ({
        ...initialPlayerState,
        volume: s.volume,
        muted: s.muted,
        loadId: s.loadId + 1,
      })),

    setProgress: (position, duration) => set({ position, duration }),
    setStatus: (status) => set({ status }),

    handleEnded() {
      const s = get()
      const i = nextIndex(model(s), 'auto')
      if (i !== null) return goTo(model(s), i)
      // End of the queue: stay on the last entry, ready to be played again from the start.
      set({
        wantPlaying: false,
        autoplay: false,
        status: 'paused',
        position: 0,
        startAt: 0,
        loadId: s.loadId + 1,
      })
    },

    skipUnplayable() {
      const s = get()
      const i = nextIndex(model(s), 'user')
      if (i === null) {
        set({ wantPlaying: false, status: 'error' })
        return false
      }
      goTo(model(s), i, s.wantPlaying)
      return true
    },

    restore(saved) {
      const entry = currentItem(saved)
      set({
        ...saved,
        status: entry ? 'paused' : 'idle',
        wantPlaying: false,
        autoplay: false,
        startAt: saved.position,
        duration: 0,
      })
    },
  }
})
