import { describe, expect, it, vi } from 'vitest'
import { REFILL_THRESHOLD, createRadioController, pickRefill, type RadioPlayer } from './radio'
import type { Track } from './types'

let seq = 0
const song = (artist = `ar${seq}`): Track =>
  ({
    Id: `s${++seq}`,
    Name: `Song ${seq}`,
    Artists: [artist],
    ArtistItems: [{ Id: artist }],
  }) as Track

const songs = (n: number): Track[] => Array.from({ length: n }, () => song())

function fakePlayer(initial: Track[], index = 0) {
  const state = { queue: initial.map((track) => ({ track })), index }
  const listeners = new Set<() => void>()
  const player: RadioPlayer = {
    getState: () => state,
    subscribe: (fn) => {
      listeners.add(fn)
      return () => listeners.delete(fn)
    },
    addToQueue(tracks) {
      state.queue = [...state.queue, ...tracks.map((track) => ({ track }))]
      listeners.forEach((l) => l())
    },
  }
  return {
    player,
    state,
    goTo(i: number) {
      state.index = i
      listeners.forEach((l) => l())
    },
    playSomethingElse(t: Track) {
      state.queue = [{ track: t }]
      state.index = 0
      listeners.forEach((l) => l())
    },
  }
}

const flush = () => new Promise((r) => setTimeout(r, 0))
const seed = { id: 'seed', name: 'Seed', kind: 'song' } as const

describe('pickRefill', () => {
  it('drops what was heard or queued, duplicates and over-represented artists', () => {
    const heard = song('a')
    const fresh = [song('b'), song('b'), song('b'), song('b'), song('c')]
    const out = pickRefill(
      [heard, ...fresh, fresh[0] as Track],
      new Set([heard.Id ?? '']),
      undefined,
    )
    expect(out.find((t) => t.Id === heard.Id)).toBeUndefined()
    expect(new Set(out.map((t) => t.Id)).size).toBe(out.length)
    expect(out.filter((t) => t.Artists?.[0] === 'b')).toHaveLength(3)
  })

  it('does not start with the artist that just played', () => {
    const out = pickRefill([song('a'), song('b')], new Set(), 'a')
    expect(out[0]?.Artists?.[0]).toBe('b')
  })
})

describe('radio refill', () => {
  it('refills before the queue runs out', async () => {
    const first = songs(10)
    const p = fakePlayer(first)
    const fetchMore = vi.fn(async () => songs(20))
    const publish = vi.fn()
    const radio = createRadioController({ player: p.player, fetchMore }, publish)
    radio.start(seed, first)
    // 9 songs after the current one: plenty, nothing to fetch yet.
    await flush()
    expect(fetchMore).not.toHaveBeenCalled()
    p.goTo(10 - REFILL_THRESHOLD) // 7 left: under the threshold
    await flush()
    expect(fetchMore).toHaveBeenCalledTimes(1)
    expect(fetchMore).toHaveBeenCalledWith('seed', first[2]?.Id)
    expect(p.state.queue.length).toBeGreaterThan(10)
    expect(publish).toHaveBeenLastCalledWith({ active: true, seed })
  })

  it('never queues a song twice', async () => {
    const first = songs(5)
    const p = fakePlayer(first)
    const radio = createRadioController({
      player: p.player,
      fetchMore: async () => [...first, ...songs(12)],
    })
    radio.start(seed, first)
    await flush()
    const ids = p.state.queue.map((q) => q.track.Id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('fetches once at a time', async () => {
    const first = songs(3)
    const p = fakePlayer(first)
    let release: (t: Track[]) => void = () => undefined
    const fetchMore = vi.fn(() => new Promise<Track[]>((r) => (release = r)))
    const radio = createRadioController({ player: p.player, fetchMore })
    radio.start(seed, first)
    p.goTo(1)
    p.goTo(2)
    expect(fetchMore).toHaveBeenCalledTimes(1)
    release(songs(30))
    await flush()
  })

  it('ends when the user plays something else', () => {
    const first = songs(10)
    const p = fakePlayer(first)
    const publish = vi.fn()
    const radio = createRadioController(
      { player: p.player, fetchMore: async () => songs(5) },
      publish,
    )
    radio.start(seed, first)
    p.playSomethingElse(song())
    expect(publish).toHaveBeenLastCalledWith({ active: false, seed: null })
  })

  it('gives up after repeated empty refills', async () => {
    const first = songs(3)
    const p = fakePlayer(first)
    const fetchMore = vi.fn(async () => [] as Track[])
    const publish = vi.fn()
    const radio = createRadioController({ player: p.player, fetchMore }, publish)
    radio.start(seed, first)
    // An empty refill is retried when the player moves on to the next song.
    await flush()
    p.goTo(1)
    await flush()
    p.goTo(2)
    await flush()
    expect(fetchMore).toHaveBeenCalledTimes(3)
    expect(publish).toHaveBeenLastCalledWith({ active: false, seed: null })
  })
})
