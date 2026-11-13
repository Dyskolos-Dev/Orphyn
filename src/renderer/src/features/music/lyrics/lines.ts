import type { TimedLyricLine } from '@renderer/jellyfin'

/**
 * Index of the lyric line being sung at `timeMs`: the last line whose start is <= timeMs, or -1
 * before the first line. Lines must be sorted by start time (binary search); unsynced lines
 * (startMs null) are never "current".
 */
export function findLineIndex(lines: readonly TimedLyricLine[], timeMs: number): number {
  let lo = 0
  let hi = lines.length - 1
  let found = -1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    const start = lines[mid]?.startMs
    if (start == null) return -1
    if (start <= timeMs) {
      found = mid
      lo = mid + 1
    } else hi = mid - 1
  }
  return found
}
