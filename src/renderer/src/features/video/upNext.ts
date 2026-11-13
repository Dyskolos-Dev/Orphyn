/** Seconds before the end at which the "Up Next" card appears. */
export const UP_NEXT_WINDOW_SECONDS = 15

export interface UpNextInput {
  positionSeconds: number
  durationSeconds: number
  hasNext: boolean
  dismissed: boolean
}

export interface UpNextState {
  visible: boolean
  /** Whole seconds until the next episode starts (when auto-play is on). */
  secondsLeft: number
}

/** When to show the Up Next card and what its countdown reads. */
export function upNextState(input: UpNextInput): UpNextState {
  const { positionSeconds, durationSeconds, hasNext, dismissed } = input
  if (!hasNext || dismissed || !(durationSeconds > UP_NEXT_WINDOW_SECONDS)) {
    return { visible: false, secondsLeft: 0 }
  }
  const remaining = durationSeconds - positionSeconds
  const visible = remaining <= UP_NEXT_WINDOW_SECONDS && remaining > 0
  return { visible, secondsLeft: visible ? Math.ceil(remaining) : 0 }
}
