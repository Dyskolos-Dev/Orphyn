/**
 * Position logic for the custom video PiP window (WebKitGTK, which has no native Picture in
 * Picture): the main player pauses and hands its stream over, the PiP window continues it, and
 * closing the PiP returns playback to the main window at the position the PiP reached.
 */

const TICKS_PER_SECOND = 10_000_000
/** Closer to the end than this, a hand-over starts a few seconds earlier rather than at "ended". */
const END_GUARD_SEC = 2

const finite = (n: number | undefined): n is number => typeof n === 'number' && Number.isFinite(n)

/** Where the PiP window starts, from the main player's position. */
export function handOverStart(positionSec: number, durationSec: number): number {
  if (!finite(positionSec) || positionSec < 0) return 0
  if (
    finite(durationSec) &&
    durationSec > END_GUARD_SEC &&
    positionSec > durationSec - END_GUARD_SEC
  ) {
    return durationSec - END_GUARD_SEC
  }
  return positionSec
}

export const toTicks = (seconds: number): number => Math.round(seconds * TICKS_PER_SECOND)

export interface PipOutcome {
  positionSec: number
  durationSec: number
  playing: boolean
  ended: boolean
}

/**
 * Where the main player resumes when the PiP closes. `last` is the last progress the PiP
 * reported (null when it never did: the window failed to start, so the main player simply
 * continues where it paused).
 */
export function returnPosition(
  last: PipOutcome | null,
  mainPositionSec: number,
  mainDurationSec: number,
): number {
  const max = finite(mainDurationSec) && mainDurationSec > 0 ? mainDurationSec : Infinity
  const pos = last && finite(last.positionSec) ? last.positionSec : mainPositionSec
  return Math.min(Math.max(0, finite(pos) ? pos : 0), max)
}

/** Resume in the main window only if the viewer was watching, not if they paused in the PiP. */
export function shouldResumeOnReturn(last: PipOutcome | null, wasPlayingBefore: boolean): boolean {
  if (last?.ended) return false
  return last ? last.playing : wasPlayingBefore
}
