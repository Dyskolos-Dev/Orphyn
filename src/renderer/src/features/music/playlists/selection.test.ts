import { describe, expect, it } from 'vitest'
import {
  EMPTY_SELECTION,
  clickSelection,
  pickSelected,
  pruneSelection,
  selectAll,
} from './selection'

const none = { shift: false, toggle: false }
const sorted = (s: ReadonlySet<number>) => [...s].sort((a, b) => a - b)

describe('multi-select', () => {
  it('toggles rows with Ctrl / Cmd and moves the anchor', () => {
    let s = clickSelection(EMPTY_SELECTION, 2, { shift: false, toggle: true })
    s = clickSelection(s, 5, { shift: false, toggle: true })
    expect(sorted(s.selected)).toEqual([2, 5])
    expect(s.anchor).toBe(5)
    s = clickSelection(s, 2, { shift: false, toggle: true })
    expect(sorted(s.selected)).toEqual([5])
  })

  it('selects a range with Shift, in either direction, from the anchor', () => {
    let s = clickSelection(EMPTY_SELECTION, 3, { shift: false, toggle: true })
    s = clickSelection(s, 6, { shift: true, toggle: false })
    expect(sorted(s.selected)).toEqual([3, 4, 5, 6])
    s = clickSelection(s, 1, { shift: true, toggle: false })
    expect(sorted(s.selected)).toEqual([1, 2, 3])
    expect(s.anchor).toBe(3)
  })

  it('Shift without an anchor selects just the row; Ctrl+Shift keeps the rest', () => {
    expect(
      sorted(clickSelection(EMPTY_SELECTION, 4, { shift: true, toggle: false }).selected),
    ).toEqual([4])
    let s = clickSelection(EMPTY_SELECTION, 0, { shift: false, toggle: true })
    s = clickSelection(s, 8, { shift: false, toggle: true })
    s = clickSelection(s, 6, { shift: true, toggle: true })
    expect(sorted(s.selected)).toEqual([0, 6, 7, 8])
  })

  it('a plain click clears', () => {
    const s = clickSelection(selectAll(3), 1, none)
    expect(s.selected.size).toBe(0)
    expect(s.anchor).toBeNull()
  })

  it('prunes after the list shrinks and picks in list order', () => {
    const s = selectAll(5)
    expect(pruneSelection(s, 5)).toBe(s)
    const p = pruneSelection(s, 2)
    expect(sorted(p.selected)).toEqual([0, 1])
    expect(pickSelected(['a', 'b', 'c'], { selected: new Set([2, 0]), anchor: 0 })).toEqual([
      'a',
      'c',
    ])
  })
})
