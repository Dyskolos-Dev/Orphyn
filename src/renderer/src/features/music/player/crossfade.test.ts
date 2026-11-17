import { describe, expect, it } from 'vitest'
import {
  fadeCurve,
  fadeGains,
  isAlbumContinuation,
  planFade,
  type FadePlanInput,
} from './crossfade'
import type { Track } from './types'

const track = (over: Partial<Track> = {}, seconds = 200): Track => ({
  Id: 'x',
  AlbumId: 'a1',
  RunTimeTicks: seconds * 1e7,
  ...over,
})

const input = (over: Partial<FadePlanInput> = {}): FadePlanInput => ({
  settings: { enabled: true, seconds: 6, skipSameAlbum: true },
  current: track({ AlbumId: 'a1' }),
  next: track({ AlbumId: 'a2' }),
  repeat: 'off',
  casting: false,
  ...over,
})

describe('equal-power curves', () => {
  it('goes from silence to full and keeps the power constant', () => {
    expect(fadeGains(0)).toEqual({ in: 0, out: 1 })
    expect(fadeGains(1).in).toBeCloseTo(1)
    expect(fadeGains(1).out).toBeCloseTo(0)
    for (const p of [0.1, 0.25, 0.5, 0.8]) {
      const g = fadeGains(p)
      expect(g.in ** 2 + g.out ** 2).toBeCloseTo(1)
    }
    expect(fadeGains(0.5).in).toBeCloseTo(Math.SQRT1_2)
  })

  it('samples the curves from the given progress to the end', () => {
    const up = fadeCurve('in')
    const down = fadeCurve('out')
    expect(up[0]).toBe(0)
    expect(up[up.length - 1]).toBeCloseTo(1)
    expect(down[0]).toBe(1)
    expect(down[down.length - 1]).toBeCloseTo(0)
    const resumed = fadeCurve('in', 0.5, 8)
    expect(resumed[0]).toBeCloseTo(Math.SQRT1_2)
    expect(resumed[7]).toBeCloseTo(1)
  })
})

describe('planFade', () => {
  it('uses the wanted length between two different albums', () => {
    expect(planFade(input())).toBe(6)
  })

  it('does nothing when disabled', () => {
    expect(planFade(input({ settings: { enabled: false, seconds: 6, skipSameAlbum: true } }))).toBe(
      0,
    )
  })

  it('does not fade on repeat-one, nor when casting', () => {
    expect(planFade(input({ repeat: 'one' }))).toBe(0)
    expect(planFade(input({ casting: true }))).toBe(0)
    expect(planFade(input({ repeat: 'all' }))).toBe(6)
  })

  it('keeps consecutive tracks of an album gapless unless the option is off', () => {
    const a = track({ AlbumId: 'a1', IndexNumber: 3, ParentIndexNumber: 1 })
    const b = track({ AlbumId: 'a1', IndexNumber: 4, ParentIndexNumber: 1 })
    expect(isAlbumContinuation(a, b)).toBe(true)
    expect(planFade(input({ current: a, next: b }))).toBe(0)
    expect(
      planFade(
        input({
          current: a,
          next: b,
          settings: { enabled: true, seconds: 6, skipSameAlbum: false },
        }),
      ),
    ).toBe(6)
  })

  it('still fades between shuffled tracks of the same album', () => {
    const a = track({ AlbumId: 'a1', IndexNumber: 3 })
    const c = track({ AlbumId: 'a1', IndexNumber: 9 })
    expect(isAlbumContinuation(a, c)).toBe(false)
    expect(planFade(input({ current: a, next: c }))).toBe(6)
  })

  it('shortens the fade for tracks shorter than twice the length', () => {
    expect(planFade(input({ next: track({ AlbumId: 'a2' }, 8) }))).toBe(4)
    expect(planFade(input({ current: track({ AlbumId: 'a1' }, 5) }))).toBe(2.5)
    expect(planFade(input({ currentDuration: 7 }))).toBe(3.5)
  })

  it('skips the fade when it would be inaudible', () => {
    expect(planFade(input({ next: track({ AlbumId: 'a2' }, 0.8) }))).toBe(0)
  })

  it('clamps the wanted length to 1-12 s', () => {
    expect(planFade(input({ settings: { enabled: true, seconds: 40, skipSameAlbum: true } }))).toBe(
      12,
    )
    expect(planFade(input({ settings: { enabled: true, seconds: 0, skipSameAlbum: true } }))).toBe(
      1,
    )
  })
})
