import { describe, expect, it } from 'vitest'
import type { SyncPlayCommand } from '@renderer/jellyfin'
import {
  DRIFT,
  commandKey,
  describeMembers,
  driftAction,
  expectedPosition,
  likelyAuthor,
  memberInitials,
  planCommand,
  queueMove,
  sameItems,
  waitingFor,
} from './logic'

const cmd = (
  command: SyncPlayCommand['command'],
  when: number,
  seconds: number,
): SyncPlayCommand => ({
  groupId: 'g',
  playlistItemId: 'p',
  when,
  positionTicks: seconds * 10_000_000,
  command,
  emittedAt: when - 1000,
})

describe('planCommand (scheduling at When)', () => {
  // Server clock 2 s ahead of ours.
  const offset = 2000

  it('schedules an Unpause at the local equivalent of When', () => {
    const plan = planCommand(cmd('Unpause', 12_000, 30), 9_000, offset)
    expect(plan).toEqual({ atLocal: 10_000, positionSeconds: 30, late: false })
  })

  it('starts a late Unpause now, further into the item', () => {
    const plan = planCommand(cmd('Unpause', 12_000, 30), 10_750, offset)
    expect(plan.late).toBe(true)
    expect(plan.atLocal).toBe(10_750)
    expect(plan.positionSeconds).toBeCloseTo(30.75)
  })

  it('pauses at When on the given frame, or at once when late', () => {
    expect(planCommand(cmd('Pause', 12_000, 42), 9_500, offset)).toEqual({
      atLocal: 10_000,
      positionSeconds: 42,
      late: false,
    })
    expect(planCommand(cmd('Pause', 12_000, 42), 11_000, offset)).toEqual({
      atLocal: 11_000,
      positionSeconds: 42,
      late: true,
    })
  })

  it('applies Seek and Stop at once', () => {
    expect(planCommand(cmd('Seek', 15_000, 90), 9_000, offset)).toMatchObject({
      atLocal: 9_000,
      positionSeconds: 90,
    })
    expect(planCommand(cmd('Stop', 1_000, 0), 9_000, offset).atLocal).toBe(9_000)
  })

  it('keys commands for de-duplication', () => {
    expect(commandKey(cmd('Pause', 1, 2))).toBe(commandKey(cmd('Pause', 1, 2)))
    expect(commandKey(cmd('Pause', 1, 2))).not.toBe(commandKey(cmd('Pause', 1, 3)))
  })

  it('expected position moves with the server clock', () => {
    const last = { when: 100_000, positionTicks: 60 * 10_000_000 }
    expect(expectedPosition(last, 102_500)).toBeCloseTo(62.5)
    expect(expectedPosition(last, 99_000)).toBe(60) // before the start: not negative time
  })
})

describe('driftAction', () => {
  it('leaves tiny drift alone (normal speed)', () => {
    expect(driftAction(0.02, true)).toEqual({ kind: 'rate', rate: 1 })
    expect(driftAction(-DRIFT.deadBandS, true)).toEqual({ kind: 'rate', rate: 1 })
  })

  it('nudges the speed for drift under a second', () => {
    expect(driftAction(0.06, true)).toEqual({ kind: 'rate', rate: 0.97 }) // ahead: slow down
    expect(driftAction(-0.06, true)).toEqual({ kind: 'rate', rate: 1.03 }) // behind: speed up
    expect(driftAction(0.8, true)).toEqual({ kind: 'rate', rate: 0.95 }) // clamped
    expect(driftAction(-0.8, true)).toEqual({ kind: 'rate', rate: 1.05 })
  })

  it('seeks beyond a second', () => {
    expect(driftAction(1.2, true)).toEqual({ kind: 'seek' })
    expect(driftAction(-4, true)).toEqual({ kind: 'seek' })
  })

  it('players without speed control only seek on a larger drift', () => {
    expect(driftAction(0.3, false)).toEqual({ kind: 'none' })
    expect(driftAction(-0.7, false)).toEqual({ kind: 'seek' })
  })

  it('ignores nonsense', () => {
    expect(driftAction(NaN, true)).toEqual({ kind: 'none' })
  })
})

describe('members', () => {
  it('shows everyone ready while the group plays', () => {
    expect(describeMembers(['alex', 'me', 'sam'], 'me', true, 'Playing')).toEqual([
      { name: 'me', isSelf: true, state: 'ready' },
      { name: 'alex', isSelf: false, state: 'ready' },
      { name: 'sam', isSelf: false, state: 'ready' },
    ])
  })

  it('names the one other member the group waits for', () => {
    const members = describeMembers(['me', 'sam'], 'me', true, 'Waiting')
    expect(members[1]).toEqual({ name: 'sam', isSelf: false, state: 'buffering' })
    expect(waitingFor(members)).toBe('sam')
  })

  it('does not guess among several members, nor when we load ourselves', () => {
    const many = describeMembers(['me', 'sam', 'alex'], 'me', true, 'Waiting')
    expect(many.slice(1).map((m) => m.state)).toEqual(['waiting', 'waiting'])
    expect(waitingFor(many)).toBeNull()
    const self = describeMembers(['me', 'sam'], 'me', false, 'Waiting')
    expect(self.map((m) => m.state)).toEqual(['buffering', 'waiting'])
  })

  it('keeps a second session of the same user', () => {
    expect(describeMembers(['me', 'me'], 'me', true, 'Paused')).toHaveLength(2)
  })

  it('guesses the author only with one other member', () => {
    expect(likelyAuthor(['me', 'sam'], 'me')).toBe('sam')
    expect(likelyAuthor(['me', 'sam', 'alex'], 'me')).toBeNull()
  })

  it('initials', () => {
    expect(memberInitials('Sam Rivera')).toBe('SR')
    expect(memberInitials('alex')).toBe('AL')
    expect(memberInitials('  ')).toBe('?')
  })
})

describe('queue helpers', () => {
  it('compares the local queue with the group one', () => {
    const group = [
      { itemId: 'a', playlistItemId: '1' },
      { itemId: 'b', playlistItemId: '2' },
    ]
    expect(sameItems(['a', 'b'], group)).toBe(true)
    expect(sameItems(['b', 'a'], group)).toBe(false)
    expect(sameItems(['a'], group)).toBe(false)
  })

  it('classifies a move of the current entry', () => {
    expect(queueMove(2, 3, 10)).toBe('next')
    expect(queueMove(9, 0, 10)).toBe('next') // repeat all wraps
    expect(queueMove(3, 2, 10)).toBe('previous')
    expect(queueMove(1, 7, 10)).toBe('jump')
    expect(queueMove(4, 4, 10)).toBe('none')
  })
})
