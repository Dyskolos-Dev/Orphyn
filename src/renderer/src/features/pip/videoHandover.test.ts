import { describe, expect, it } from 'vitest'
import { handOverStart, returnPosition, shouldResumeOnReturn, toTicks } from './videoHandover'

describe('handOverStart', () => {
  it('continues at the current position', () => {
    expect(handOverStart(125.4, 3600)).toBe(125.4)
  })

  it('never starts negative or on garbage', () => {
    expect(handOverStart(-3, 100)).toBe(0)
    expect(handOverStart(Number.NaN, 100)).toBe(0)
  })

  it('does not start on the very last seconds', () => {
    expect(handOverStart(99.5, 100)).toBe(98)
  })

  it('converts seconds to Jellyfin ticks', () => {
    expect(toTicks(1.5)).toBe(15_000_000)
  })
})

const pip = (positionSec: number, over = {}) => ({
  positionSec,
  durationSec: 200,
  playing: true,
  ended: false,
  ...over,
})

describe('returnPosition', () => {
  it('returns at the position the PiP reached', () => {
    expect(returnPosition(pip(80), 50, 200)).toBe(80)
  })

  it('falls back to where the main player paused when the PiP never reported', () => {
    expect(returnPosition(null, 50, 200)).toBe(50)
    expect(returnPosition(pip(Number.NaN), 50, 200)).toBe(50)
  })

  it('stays inside the video', () => {
    expect(returnPosition(pip(500), 50, 200)).toBe(200)
    expect(returnPosition(pip(-4), 50, 200)).toBe(0)
  })
})

describe('shouldResumeOnReturn', () => {
  it('resumes when the PiP was playing, stays paused when the viewer paused there', () => {
    expect(shouldResumeOnReturn(pip(10), false)).toBe(true)
    expect(shouldResumeOnReturn(pip(10, { playing: false }), true)).toBe(false)
  })

  it('does not resume a finished video, and keeps the prior intent without any report', () => {
    expect(shouldResumeOnReturn(pip(10, { ended: true }), true)).toBe(false)
    expect(shouldResumeOnReturn(null, true)).toBe(true)
    expect(shouldResumeOnReturn(null, false)).toBe(false)
  })
})
