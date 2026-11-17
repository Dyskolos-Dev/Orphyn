/**
 * Keyboard navigation of the palette list as a pure reducer. The list is flat; groups are given as
 * the start index of each section so Tab can jump between them.
 */

export interface NavState {
  query: string
  /** Highlighted row. */
  index: number
  /** Who moved the highlight last: the keyboard scrolls it into view, the pointer does not. */
  source: 'keyboard' | 'pointer'
}

export type NavAction =
  | { type: 'setQuery'; query: string }
  | { type: 'move'; delta: number; count: number }
  | { type: 'edge'; to: 'first' | 'last'; count: number }
  | { type: 'cycleGroup'; direction: 1 | -1; groupStarts: readonly number[] }
  | { type: 'hover'; index: number }
  | { type: 'clamp'; count: number }
  | { type: 'reset' }

export const initialNavState: NavState = { query: '', index: 0, source: 'keyboard' }

/** Index of the group containing `index` (last start <= index). */
export function groupOf(index: number, groupStarts: readonly number[]): number {
  let g = 0
  groupStarts.forEach((start, i) => {
    if (start <= index) g = i
  })
  return g
}

export function navReducer(state: NavState, action: NavAction): NavState {
  switch (action.type) {
    case 'setQuery':
      return action.query === state.query
        ? state
        : { query: action.query, index: 0, source: 'keyboard' }
    case 'move': {
      if (action.count <= 0) return { ...state, index: 0, source: 'keyboard' }
      // Wraps around: Down on the last row goes back to the first.
      const next = (((state.index + action.delta) % action.count) + action.count) % action.count
      return { ...state, index: next, source: 'keyboard' }
    }
    case 'edge':
      return {
        ...state,
        index: action.to === 'first' ? 0 : Math.max(0, action.count - 1),
        source: 'keyboard',
      }
    case 'cycleGroup': {
      const starts = action.groupStarts
      if (starts.length === 0) return state
      const current = groupOf(state.index, starts)
      const next = (current + action.direction + starts.length) % starts.length
      return { ...state, index: starts[next] as number, source: 'keyboard' }
    }
    case 'hover':
      return action.index === state.index && state.source === 'pointer'
        ? state
        : { ...state, index: action.index, source: 'pointer' }
    case 'clamp':
      return state.index < Math.max(1, action.count)
        ? state
        : { ...state, index: Math.max(0, action.count - 1) }
    case 'reset':
      return initialNavState
  }
}
