import { describe, expect, it } from 'vitest'
import {
  append,
  clearUpcoming,
  createQueue,
  insertNext,
  move,
  nextIndex,
  previousAction,
  removeAt,
  setShuffle,
} from './queue'
import type { QueueModel, Track } from './types'

const tracks = (n: number): Track[] =>
  Array.from({ length: n }, (_, i) => ({ Id: `t${i}`, Name: `Track ${i}` }))
const ids = (m: QueueModel) => m.queue.map((q) => q.track.Id)
const lcg =
  (seed = 1) =>
  () =>
    (seed = (seed * 16807) % 2147483647) / 2147483647
const base = (n: number, start = 0) =>
  createQueue(tracks(n), start, { shuffle: false, repeat: 'off' })

describe('queue operations', () => {
  it('starts at the requested index and clamps it', () => {
    expect(base(5, 2).index).toBe(2)
    expect(base(5, 99).index).toBe(4)
    expect(createQueue([], 0, { shuffle: false, repeat: 'off' }).index).toBe(-1)
  })

  it('playNext inserts right after the current entry, addToQueue appends', () => {
    let m = base(3, 1)
    m = insertNext(m, [{ Id: 'x' }, { Id: 'y' }])
    expect(ids(m)).toEqual(['t0', 't1', 'x', 'y', 't2'])
    m = append(m, [{ Id: 'z' }])
    expect(ids(m)).toEqual(['t0', 't1', 'x', 'y', 't2', 'z'])
    expect(m.index).toBe(1)
  })

  it('inserting into an empty queue makes the first entry current', () => {
    const m = insertNext(createQueue([], 0, { shuffle: false, repeat: 'off' }), tracks(2))
    expect(m.index).toBe(0)
  })

  it('keeps the current entry when removing before it, and moves on when removing it', () => {
    const m = base(4, 2)
    const before = removeAt(m, 0)
    expect(before.model.index).toBe(1)
    expect(before.model.queue[before.model.index]?.track.Id).toBe('t2')
    expect(before.removedCurrent).toBe(false)

    const current = removeAt(m, 2)
    expect(current.removedCurrent).toBe(true)
    expect(current.model.queue[current.model.index]?.track.Id).toBe('t3')

    const last = removeAt(base(3, 2), 2)
    expect(last.model.index).toBe(1)
    expect(removeAt(base(1), 0).model.index).toBe(-1)
  })

  it('reorders and follows the current entry', () => {
    const m = base(5, 2)
    expect(ids(move(m, 0, 4))).toEqual(['t1', 't2', 't3', 't4', 't0'])
    expect(move(m, 0, 4).index).toBe(1) // moved from before to after the current entry
    expect(move(m, 4, 0).index).toBe(3) // after -> before
    const cur = move(m, 2, 4)
    expect(cur.index).toBe(4)
    expect(cur.queue[cur.index]?.track.Id).toBe('t2')
    expect(move(m, 1, 1)).toBe(m)
  })

  it('clears the upcoming entries', () => {
    const m = clearUpcoming(base(5, 1))
    expect(ids(m)).toEqual(['t0', 't1'])
  })
})

describe('shuffle', () => {
  it('starts with the chosen track and keeps every track once', () => {
    const m = createQueue(tracks(20), 7, { shuffle: true, repeat: 'off' }, lcg())
    expect(m.index).toBe(0)
    expect(m.queue[0]?.track.Id).toBe('t7')
    expect([...ids(m)].sort()).toEqual(ids(base(20)).sort())
    expect(ids(m)).not.toEqual(ids(base(20)))
  })

  it('picks a random first track when no start index is given', () => {
    const m = createQueue(tracks(10), undefined, { shuffle: true, repeat: 'off' }, () => 0.5)
    expect(m.queue[0]?.track.Id).toBe('t5')
  })

  it('shuffling on a running queue keeps played entries and the current one in place', () => {
    const m = setShuffle(base(10, 3), true, lcg(3))
    expect(ids(m).slice(0, 4)).toEqual(['t0', 't1', 't2', 't3'])
    expect(m.index).toBe(3)
    expect([...ids(m).slice(4)].sort()).toEqual(['t4', 't5', 't6', 't7', 't8', 't9'])
  })

  it('turning shuffle off restores the original order around the current entry', () => {
    let m = createQueue(tracks(8), 2, { shuffle: true, repeat: 'off' }, lcg(5))
    m = { ...m, index: 3 }
    const currentId = m.queue[3]?.track.Id
    m = setShuffle(m, false)
    expect(ids(m)).toEqual(ids(base(8)))
    expect(m.queue[m.index]?.track.Id).toBe(currentId)
    expect(m.original).toBeNull()
  })

  it('entries added while shuffled survive turning shuffle off', () => {
    let m = createQueue(tracks(4), 0, { shuffle: true, repeat: 'off' }, lcg(2))
    m = append(m, [{ Id: 'late' }])
    m = removeAt(m, 1).model
    m = setShuffle(m, false)
    expect(ids(m)).toHaveLength(4)
    expect(ids(m)).toContain('late')
  })
})

describe('next / repeat', () => {
  it('moves forward and ends the queue with repeat off', () => {
    expect(nextIndex(base(3, 0), 'auto')).toBe(1)
    expect(nextIndex(base(3, 2), 'auto')).toBeNull()
    expect(nextIndex(base(3, 2), 'user')).toBeNull()
  })

  it('repeat all wraps around', () => {
    const m = { ...base(3, 2), repeat: 'all' as const }
    expect(nextIndex(m, 'auto')).toBe(0)
    expect(nextIndex(m, 'user')).toBe(0)
  })

  it('repeat one loops on its own but a user next still advances', () => {
    const m = { ...base(3, 1), repeat: 'one' as const }
    expect(nextIndex(m, 'auto')).toBe(1)
    expect(nextIndex(m, 'user')).toBe(2)
  })

  it('has nothing next in an empty queue', () => {
    expect(nextIndex(base(0), 'auto')).toBeNull()
  })
})

describe('previous', () => {
  it('restarts the track after more than 3 seconds', () => {
    expect(previousAction(base(3, 2), 3.1)).toEqual({ type: 'restart' })
  })
  it('goes back within the first 3 seconds', () => {
    expect(previousAction(base(3, 2), 3)).toEqual({ type: 'go', index: 1 })
    expect(previousAction(base(3, 2), 0)).toEqual({ type: 'go', index: 1 })
  })
  it('restarts on the first entry', () => {
    expect(previousAction(base(3, 0), 1)).toEqual({ type: 'restart' })
  })
})
