import { describe, expect, it } from 'vitest'
import { findLineIndex } from './lines'

const lines = [
  { text: 'a', startMs: 1000 },
  { text: 'b', startMs: 5000 },
  { text: 'c', startMs: 5000 },
  { text: 'd', startMs: 9000 },
]

describe('findLineIndex', () => {
  it('is -1 before the first line', () => {
    expect(findLineIndex(lines, 0)).toBe(-1)
    expect(findLineIndex(lines, 999)).toBe(-1)
  })
  it('finds the line being sung', () => {
    expect(findLineIndex(lines, 1000)).toBe(0)
    expect(findLineIndex(lines, 4999)).toBe(0)
    expect(findLineIndex(lines, 9000)).toBe(3)
    expect(findLineIndex(lines, 999999)).toBe(3)
  })
  it('takes the last of lines sharing a start time', () => {
    expect(findLineIndex(lines, 5000)).toBe(2)
  })
  it('handles empty and unsynced lyrics', () => {
    expect(findLineIndex([], 5)).toBe(-1)
    expect(findLineIndex([{ text: 'x', startMs: null }], 5)).toBe(-1)
  })
})
