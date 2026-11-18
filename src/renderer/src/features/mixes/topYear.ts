/** The "Top titres <année>" mix: the year's most played songs, built from the played history. */
import type { QueryClient } from '@tanstack/react-query'
import { loadTopSongs } from '../stats/useStats'
import type { Mix, Track } from './types'

/** Fewer songs than this is not worth a mix of its own. */
export const MIN_TOP_TRACKS = 5

export const topYearMixId = (year: number): string => `topYear-${year}`

export function makeTopYearMix(year: number, tracks: Track[], now = Date.now()): Mix {
  const artists: string[] = []
  for (const t of tracks) {
    const a = t.ArtistItems?.[0]?.Name ?? t.Artists?.[0] ?? t.AlbumArtist
    if (a && !artists.includes(a)) artists.push(a)
    if (artists.length === 3) break
  }
  return {
    id: topYearMixId(year),
    kind: 'topYear',
    year,
    hue: 205,
    generatedAt: now,
    artists,
    tracks,
  }
}

/** The mix of the given year, or null when it has too few plays (or the server does not answer). */
export async function loadTopYearMix(
  qc: QueryClient,
  accountId: string,
  userId: string,
  year = new Date().getFullYear(),
): Promise<Mix | null> {
  try {
    const tracks = await loadTopSongs(qc, accountId, userId, year)
    return tracks.length >= MIN_TOP_TRACKS ? makeTopYearMix(year, tracks) : null
  } catch {
    return null
  }
}
