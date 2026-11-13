import { describe, expect, it } from 'vitest'
import {
  EMPTY_MUSIC_PIP,
  extrapolatePosition,
  musicPipChanged,
  musicPipReducer,
  type MusicPipState,
} from './protocol'

const snap = (over: Partial<MusicPipState> = {}): MusicPipState => ({
  hasTrack: true,
  trackId: 't1',
  title: 'Song',
  artist: 'Artist',
  playing: true,
  position: 10,
  duration: 100,
  favorite: false,
  at: 1000,
  ...over,
})

describe('musicPipReducer', () => {
  it('takes a newer snapshot', () => {
    const next = musicPipReducer(EMPTY_MUSIC_PIP, { type: 'snapshot', state: snap() })
    expect(next.title).toBe('Song')
  })

  it('drops a stale snapshot that arrives late', () => {
    const held = snap({ at: 2000, title: 'New' })
    expect(musicPipReducer(held, { type: 'snapshot', state: snap({ at: 1500 }) })).toBe(held)
  })

  it('accepts a snapshot sent at the same instant (a state change in the same tick)', () => {
    const held = snap({ at: 2000 })
    const next = musicPipReducer(held, {
      type: 'snapshot',
      state: snap({ at: 2000, favorite: true }),
    })
    expect(next.favorite).toBe(true)
  })

  it('clears back to the empty state', () => {
    expect(musicPipReducer(snap(), { type: 'clear' })).toBe(EMPTY_MUSIC_PIP)
  })
})

describe('extrapolatePosition', () => {
  it('advances while playing and stops at the duration', () => {
    expect(extrapolatePosition(snap(), 4000)).toBe(13)
    expect(extrapolatePosition(snap({ position: 99 }), 4000)).toBe(100)
  })

  it('stays put while paused, and never goes backwards on clock skew', () => {
    expect(extrapolatePosition(snap({ playing: false }), 9000)).toBe(10)
    expect(extrapolatePosition(snap(), 500)).toBe(10)
  })
})

describe('musicPipChanged', () => {
  it('ignores the playing clock but sees track, state and favourite changes', () => {
    expect(musicPipChanged(snap(), snap({ position: 55, at: 9000 }))).toBe(false)
    expect(musicPipChanged(snap(), snap({ playing: false }))).toBe(true)
    expect(musicPipChanged(snap(), snap({ trackId: 't2' }))).toBe(true)
    expect(musicPipChanged(snap(), snap({ favorite: true }))).toBe(true)
    expect(musicPipChanged(snap({ duration: 0 }), snap({ duration: 100 }))).toBe(true)
  })
})
