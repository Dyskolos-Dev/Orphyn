import { beforeEach, describe, expect, it } from 'vitest'
import {
  deserializePlayer,
  loadPlayer,
  MAX_PERSISTED,
  savePlayer,
  serializePlayer,
  STORAGE_KEY,
} from './persistence'
import { createQueue } from './queue'
import type { Track } from './types'

const tracks = (n: number): Track[] =>
  Array.from({ length: n }, (_, i) => ({
    Id: `t${i}`,
    Name: `Track ${i}`,
    Artists: ['A'],
    RunTimeTicks: 1e9,
    Overview: 'dropped',
  }))

const snap = (n: number, shuffle = false) => ({
  ...createQueue(tracks(n), 1, { shuffle, repeat: 'all' }, () => 0.3),
  position: 42.5,
  volume: 0.6,
  muted: true,
})

describe('player persistence', () => {
  beforeEach(() => localStorage.clear())

  it('round-trips queue, position, shuffle, repeat and volume', () => {
    savePlayer(snap(4))
    const back = loadPlayer()
    expect(back?.queue.map((q) => q.track.Id)).toEqual(['t0', 't1', 't2', 't3'])
    expect(back?.index).toBe(1)
    expect(back?.position).toBe(42.5)
    expect(back?.repeat).toBe('all')
    expect(back?.volume).toBe(0.6)
    expect(back?.muted).toBe(true)
  })

  it('keeps the original order of a shuffled queue', () => {
    const s = snap(6, true)
    savePlayer(s)
    const back = loadPlayer()
    expect(back?.shuffle).toBe(true)
    expect(back?.original?.map((q) => q.track.Id)).toEqual(s.original?.map((q) => q.track.Id))
    expect(back?.original?.[0]).toBe(back?.queue.find((q) => q.uid === back.original?.[0]?.uid))
  })

  it('stores lean tracks only', () => {
    expect(JSON.stringify(serializePlayer(snap(2)))).not.toContain('dropped')
  })

  it('cuts huge queues around the current entry', () => {
    const big = { ...snap(MAX_PERSISTED + 200), index: 450 }
    const out = serializePlayer(big)
    expect(out.queue).toHaveLength(MAX_PERSISTED)
    expect(out.queue[out.index]?.track.Id).toBe('t450')
  })

  it('ignores corrupt or unknown data', () => {
    localStorage.setItem(STORAGE_KEY, '{nope')
    expect(loadPlayer()).toBeNull()
    expect(deserializePlayer({ version: 99, queue: [] })).toBeNull()
    expect(deserializePlayer({ version: 1, queue: [] })).toBeNull()
    expect(deserializePlayer(null)).toBeNull()
  })

  it('clamps out-of-range values', () => {
    const raw = {
      ...serializePlayer(snap(3)),
      index: 99,
      volume: 7,
      repeat: 'bogus',
      position: -4,
    }
    const back = deserializePlayer(raw)
    expect(back?.index).toBe(2)
    expect(back?.volume).toBe(1)
    expect(back?.repeat).toBe('off')
    expect(back?.position).toBe(0)
  })
})
