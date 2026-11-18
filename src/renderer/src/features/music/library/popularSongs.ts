import type { BaseItemDto } from '@renderer/jellyfin'

/** Qualifiers that never change which song it is: "(Remastered 2011)", "- Live", "feat. X"… */
const NOISE =
  /\b(?:remaster(?:ed)?|re-?master|live|version|mono|stereo|radio edit|single edit|album version|bonus track|deluxe|explicit|clean|acoustic version)\b/i
const FEAT_TAIL = /\s*[([]?\s*\b(?:feat|ft|featuring)\b\.?\s.*$/i

/** Comparable form of a song title: case, accents, bracketed qualifiers, features and punctuation gone. */
export function normalizeTitle(title: string): string {
  let t = title.normalize('NFD').replace(/[̀-ͯ]/g, '')
  // "(Remastered 2011)" / "[Live]" / "(feat. X)": only brackets holding a qualifier are dropped.
  t = t.replace(/[([][^)\]]*[)\]]/g, (m) => (NOISE.test(m) || FEAT_TAIL.test(m) ? ' ' : m))
  // "Song - Remastered 2011" / "Song - Live at Wembley": a dash tail made of a qualifier.
  t = t.replace(/\s[-–—]\s(.*)$/, (m, tail: string) => (NOISE.test(tail) ? ' ' : m))
  t = t.replace(FEAT_TAIL, ' ')
  return t
    .toLowerCase()
    .replace(/[’‘`´]/g, "'")
    .replace(/[^\p{L}\p{N}]+/gu, '')
}

const plays = (s: BaseItemDto): number => s.UserData?.PlayCount ?? 0

/**
 * The artist's popular songs that exist in the library: songs matching the online ranking come
 * first, in that ranking's order; the rest is filled by the library's own play count. A song title
 * is listed once (the most played version wins).
 */
export function pickPopularSongs(
  songs: readonly BaseItemDto[],
  rankedTitles: readonly string[],
  limit: number,
): BaseItemDto[] {
  const best = new Map<string, BaseItemDto>()
  for (const s of songs) {
    const key = normalizeTitle(s.Name ?? '')
    if (!key) continue
    const cur = best.get(key)
    if (!cur || plays(s) > plays(cur)) best.set(key, s)
  }
  const out: BaseItemDto[] = []
  const used = new Set<string>()
  for (const title of rankedTitles) {
    const key = normalizeTitle(title)
    const hit = best.get(key)
    if (hit && !used.has(key)) {
      used.add(key)
      out.push(hit)
    }
  }
  const rest = [...best.entries()]
    .filter(([k]) => !used.has(k))
    .map(([, s]) => s)
    .sort((a, b) => plays(b) - plays(a))
  return [...out, ...rest].slice(0, limit)
}
