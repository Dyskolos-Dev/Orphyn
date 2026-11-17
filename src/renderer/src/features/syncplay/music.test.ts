import { describe, expect, it, vi } from 'vitest'
import type { PlayerState } from '@renderer/features/music/player/store'
import { appended, insertedAfter, musicIntent, type QueueIntents } from './music'

const entry = (id: string) => ({ uid: `u-${id}-${Math.random()}`, track: { Id: id } })

function state(ids: string[], patch: Partial<PlayerState> = {}): PlayerState {
  return {
    queue: ids.map(entry),
    index: 0,
    wantPlaying: true,
    seekNonce: 0,
    position: 12,
    ...patch,
  } as unknown as PlayerState
}

function intents(): QueueIntents & Record<string, ReturnType<typeof vi.fn>> {
  return {
    play: vi.fn(),
    pause: vi.fn(),
    seek: vi.fn(),
    next: vi.fn(),
    previous: vi.fn(),
    jump: vi.fn(),
    newQueue: vi.fn(),
    queue: vi.fn(),
  }
}

describe('music queue diffs', () => {
  it('detects entries added at the end', () => {
    expect(appended(['a', 'b'], ['a', 'b', 'c', 'd'])).toEqual(['c', 'd'])
    expect(appended(['a', 'b'], ['b', 'a', 'c'])).toBeNull()
    expect(appended(['a'], ['a'])).toBeNull()
  })

  it('detects "play next" insertions after the current entry', () => {
    expect(insertedAfter(['a', 'b', 'c'], ['a', 'x', 'y', 'b', 'c'], 0)).toEqual(['x', 'y'])
    expect(insertedAfter(['a', 'b', 'c'], ['a', 'b', 'x', 'c'], 0)).toBeNull()
  })
})

describe('musicIntent (the person acting on the music player)', () => {
  it('a new queue becomes the group queue, at the current position', () => {
    const i = intents()
    const prev = state(['a', 'b'])
    musicIntent(state(['x', 'y', 'z'], { index: 1, position: 0 }), prev, true, i)
    expect(i.newQueue).toHaveBeenCalledWith(['x', 'y', 'z'], 1, 0)
  })

  it('"Add to queue" and "Play next" only add entries', () => {
    const i = intents()
    const prev = state(['a', 'b'])
    musicIntent(state(['a', 'b', 'c']), prev, true, i)
    expect(i.queue).toHaveBeenCalledWith(['c'], 'Queue')
    musicIntent(state(['a', 'n', 'b']), prev, true, i)
    expect(i.queue).toHaveBeenCalledWith(['n'], 'QueueNext')
    expect(i.newQueue).not.toHaveBeenCalled()
  })

  it('next / previous / jump', () => {
    const i = intents()
    const prev = state(['a', 'b', 'c', 'd'], { index: 1 })
    musicIntent({ ...prev, index: 2 }, prev, true, i)
    expect(i.next).toHaveBeenCalledWith('b')
    musicIntent({ ...prev, index: 0 }, prev, true, i)
    expect(i.previous).toHaveBeenCalledWith('b')
    musicIntent({ ...prev, index: 3 }, prev, true, i)
    expect(i.jump).toHaveBeenCalledWith('d')
  })

  it('seek and pause go to the group', () => {
    const i = intents()
    const prev = state(['a'])
    musicIntent({ ...prev, seekNonce: 1, position: 80 }, prev, true, i)
    expect(i.seek).toHaveBeenCalledWith(80)
    musicIntent({ ...prev, wantPlaying: false }, prev, true, i)
    expect(i.pause).toHaveBeenCalled()
  })

  it('play waits for the group', () => {
    const i = intents()
    const prev = state(['a'], { wantPlaying: false })
    musicIntent({ ...prev, wantPlaying: true }, prev, true, i)
    expect(i.play).toHaveBeenCalled()
  })

  it('while the group watches a film, only a new queue counts (music pausing for the film does not)', () => {
    const i = intents()
    const prev = state(['a'])
    musicIntent({ ...prev, wantPlaying: false }, prev, false, i)
    musicIntent({ ...prev, seekNonce: 4 }, prev, false, i)
    expect(i.pause).not.toHaveBeenCalled()
    expect(i.seek).not.toHaveBeenCalled()
    musicIntent(state(['q']), prev, false, i)
    expect(i.newQueue).toHaveBeenCalledWith(['q'], 0, 12)
  })
})
