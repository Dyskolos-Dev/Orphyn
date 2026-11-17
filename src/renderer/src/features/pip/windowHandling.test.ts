import { describe, expect, it } from 'vitest'
import { COMPACT_WIDTH, LARGE_WIDTH, pastThreshold, toggledWidth } from './windowHandling'

describe('window handling', () => {
  it('only starts a drag once the pointer has really moved', () => {
    expect(pastThreshold(1, 2)).toBe(false)
    expect(pastThreshold(3, 3)).toBe(true)
  })

  it('double-click switches between the compact and the large size', () => {
    expect(toggledWidth(320)).toBe(LARGE_WIDTH)
    expect(toggledWidth(LARGE_WIDTH)).toBe(COMPACT_WIDTH)
    expect(toggledWidth(COMPACT_WIDTH)).toBe(LARGE_WIDTH)
  })
})
