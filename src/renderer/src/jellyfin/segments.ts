/** Media segments (Jellyfin 10.10+): intro, outro/credits, recap... of an item, in seconds. */
import type { Api } from '@jellyfin/sdk'
import { jfRequest } from './client'

export type MediaSegmentType = 'Intro' | 'Outro' | 'Recap' | 'Preview' | 'Commercial'

export interface MediaSegment {
  type: MediaSegmentType
  /** Seconds from the start of the item. */
  start: number
  end: number
}

interface SegmentDto {
  Type?: string | null
  StartTicks?: number | null
  EndTicks?: number | null
}

const TICKS_PER_SECOND = 10_000_000
const KNOWN: readonly string[] = ['Intro', 'Outro', 'Recap', 'Preview', 'Commercial']

/** Keeps the segments we understand with a valid, non-empty range, sorted by start. */
export function normalizeSegments(items: readonly SegmentDto[] | null | undefined): MediaSegment[] {
  const out: MediaSegment[] = []
  for (const s of items ?? []) {
    if (!s.Type || !KNOWN.includes(s.Type)) continue
    const start = (s.StartTicks ?? 0) / TICKS_PER_SECOND
    const end = (s.EndTicks ?? 0) / TICKS_PER_SECOND
    if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) continue
    out.push({ type: s.Type as MediaSegmentType, start, end })
  }
  return out.sort((a, b) => a.start - b.start)
}

/**
 * GET /MediaSegments/{id}. Servers older than 10.10 (404) or without the feature answer with an
 * error: that is "no segments", never a failure.
 */
export async function fetchMediaSegments(api: Api, itemId: string): Promise<MediaSegment[]> {
  try {
    const r = await jfRequest<{ Items?: SegmentDto[] } | undefined>(
      api,
      'GET',
      `/MediaSegments/${itemId}`,
      { allowStatus: [400, 404, 405, 501] },
    )
    return normalizeSegments(r?.Items)
  } catch {
    return []
  }
}

/**
 * The first segment of one of `types` that contains `seconds` (start included, end excluded).
 * A segment that ends within `tail` seconds is already over for the viewer.
 */
export function segmentAt(
  segments: readonly MediaSegment[] | undefined,
  seconds: number,
  types: readonly MediaSegmentType[],
  tail = 0,
): MediaSegment | undefined {
  return segments?.find(
    (s) => types.includes(s.type) && seconds >= s.start && seconds < s.end - tail,
  )
}
