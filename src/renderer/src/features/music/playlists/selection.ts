/** Multi-select logic for track lists (Shift = range, Ctrl / Cmd = toggle). Indexes, not ids: a playlist may repeat a track. */

export interface SelectionState {
  selected: ReadonlySet<number>
  /** Row the next Shift-click extends from. */
  anchor: number | null
}

export const EMPTY_SELECTION: SelectionState = { selected: new Set(), anchor: null }

export interface ClickModifiers {
  shift: boolean
  /** Ctrl on Windows / Linux, Cmd on macOS. */
  toggle: boolean
}

/** New selection after a modified click on row `index`. A plain click clears the selection. */
export function clickSelection(
  state: SelectionState,
  index: number,
  mods: ClickModifiers,
): SelectionState {
  if (mods.shift) {
    const anchor = state.anchor ?? index
    const [from, to] = anchor <= index ? [anchor, index] : [index, anchor]
    const next = new Set(mods.toggle ? state.selected : [])
    for (let i = from; i <= to; i++) next.add(i)
    return { selected: next, anchor }
  }
  if (mods.toggle) {
    const next = new Set(state.selected)
    if (next.has(index)) next.delete(index)
    else next.add(index)
    return { selected: next, anchor: index }
  }
  return EMPTY_SELECTION
}

export const selectAll = (count: number): SelectionState => ({
  selected: new Set(Array.from({ length: count }, (_, i) => i)),
  anchor: count > 0 ? 0 : null,
})

/** Drops indexes past the end (the list shrank). Returns the same object when nothing changed. */
export function pruneSelection(state: SelectionState, count: number): SelectionState {
  if ([...state.selected].every((i) => i < count)) return state
  const selected = new Set([...state.selected].filter((i) => i < count))
  return { selected, anchor: state.anchor !== null && state.anchor < count ? state.anchor : null }
}

/** Selected items in list order. */
export function pickSelected<T>(items: readonly T[], state: SelectionState): T[] {
  return items.filter((_, i) => state.selected.has(i))
}
