import { segmentAt, type MediaSegment } from '@renderer/jellyfin'

/** What the pill at the bottom right of the player offers. */
export type SkipKind = 'intro' | 'recap' | 'credits' | 'nextEpisode'

export interface SkipPill {
  kind: SkipKind
  segment: MediaSegment
}

export interface SkipInput {
  segments: readonly MediaSegment[] | undefined
  positionSeconds: number
  /** The item is an episode with an episode after it. */
  hasNext: boolean
  /** The Up Next card is on screen: it carries the "next episode" action itself. */
  upNextVisible: boolean
  /** Trailers have nothing to skip. */
  trailer?: boolean
}

/** A segment this close to its end is over for the viewer: the pill is already gone. */
export const SKIP_TAIL_SECONDS = 1

/**
 * The pill to show at `positionSeconds`, or null.
 * Intro / Recap → "Skip Intro" / "Skip Recap". Outro → "Next Episode" when an episode follows
 * (until the Up Next card takes over), else "Skip Credits". Preview and Commercial never show one.
 */
export function skipPill(input: SkipInput): SkipPill | null {
  if (input.trailer) return null
  const segment = segmentAt(
    input.segments,
    input.positionSeconds,
    ['Intro', 'Recap', 'Outro'],
    SKIP_TAIL_SECONDS,
  )
  if (!segment) return null
  if (segment.type === 'Intro') return { kind: 'intro', segment }
  if (segment.type === 'Recap') return { kind: 'recap', segment }
  if (input.hasNext) return input.upNextVisible ? null : { kind: 'nextEpisode', segment }
  return { kind: 'credits', segment }
}

/** Where skipping a segment seeks to (never past the end of the item). */
export function skipTarget(segment: MediaSegment, durationSeconds: number): number {
  return durationSeconds > 0 ? Math.min(segment.end, durationSeconds) : segment.end
}

/** Intros and recaps may be skipped automatically; credits and the next episode never are. */
export const isAutoSkippable = (pill: SkipPill): boolean =>
  pill.kind === 'intro' || pill.kind === 'recap'

/** Stable id of a segment, to skip it automatically only once. */
export const segmentKey = (itemId: string, s: MediaSegment): string =>
  `${itemId}:${s.type}:${s.start}`
