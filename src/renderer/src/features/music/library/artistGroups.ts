import type { BaseItemDto } from '@renderer/jellyfin'

/**
 * Client-side artist grouping (a pure view: nothing is ever written to the server).
 *
 * Jellyfin lists "Artist", "artist", "Artist feat. X" or "Artist & Y" as separate artists when
 * the tags differ. `artistKey` makes spelling variants collide; `splitArtistNames` cuts a
 * credit string into its artists; `groupArtists` merges everything into one entry per artist.
 */

/** Lowercase, accents stripped, punctuation dropped, spaces collapsed, leading "the" removed. */
export function artistKey(name: string): string {
  const key = name
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[’‘`´]/g, "'")
    .replace(/\s+/g, ' ')
    .trim()
  const noThe = key.replace(/^the\s+(?=\S)/, '')
  return noThe
}

/** Features are never part of a band name: always safe to cut. */
const FEAT = /\s*[([]?\s*\b(?:feat|ft|featuring)\b\.?\s+/i
/** Ambiguous separators ("Simon & Garfunkel" is one act), only trusted when the head is known. */
const JOINERS = /\s*(?:;|,|&|\+|\/|\s×\s|\s+x\s+|\s+with\s+|\s+and\s+)\s*/i

export interface SplitArtists {
  /** Every credited artist, in order, trimmed. */
  names: string[]
  /** True when the credit used feat./ft./featuring (the head is then certainly the main act). */
  featured: boolean
}

export function splitArtistCredit(credit: string): SplitArtists {
  const featured = FEAT.test(credit)
  const names: string[] = []
  const [main = '', ...rest] = credit.split(FEAT)
  for (const part of [main, ...rest]) {
    for (const p of part.split(JOINERS)) {
      const clean = p.replace(/[)\]]+\s*$/, '').trim()
      if (clean) names.push(clean)
    }
  }
  return { names: names.length > 0 ? names : [credit.trim()], featured }
}

/** The artists named in a credit string: "A feat. B & C" gives A, B and C. */
export function splitArtistNames(credit: string): string[] {
  return splitArtistCredit(credit).names
}

export interface ArtistGroup {
  /** The entry shown in the list (an album artist when there is one). */
  item: BaseItemDto
  /** Every Jellyfin artist id merged into this entry (the item's own id first). */
  ids: string[]
}

/**
 * One group per real artist.
 *
 * @param albumArtists artists that own albums (the old Artists page): these root the groups.
 * @param everyone     all artists known to the server, album artists or not; variants of a root
 *                     are attached to it, guests that match no root are left out.
 */
export function groupArtists(
  albumArtists: readonly BaseItemDto[],
  everyone: readonly BaseItemDto[] = [],
): ArtistGroup[] {
  const known = new Set<string>()
  for (const a of [...albumArtists, ...everyone]) if (a.Name) known.add(artistKey(a.Name))

  /** Key of the group a name belongs to: itself, or its head when that is the main act. */
  const groupKeyOf = (name: string): string => {
    const own = artistKey(name)
    const { names, featured } = splitArtistCredit(name)
    const head = names[0]
    if (names.length < 2 || !head) return own
    const headKey = artistKey(head)
    return featured || known.has(headKey) ? headKey : own
  }

  const groups = new Map<string, ArtistGroup>()
  const rank = (a: BaseItemDto) => {
    const composite = splitArtistCredit(a.Name ?? '').names.length > 1
    return (composite ? 2 : 0) + (a.ImageTags?.['Primary'] ? 0 : 1)
  }
  const add = (a: BaseItemDto, root: boolean) => {
    if (!a.Id || !a.Name) return
    const key = groupKeyOf(a.Name)
    const g = groups.get(key)
    if (!g) {
      if (root) groups.set(key, { item: a, ids: [a.Id] })
      return
    }
    if (!g.ids.includes(a.Id)) g.ids.push(a.Id)
    if (root && rank(a) < rank(g.item)) {
      g.item = a
      g.ids = [a.Id, ...g.ids.filter((i) => i !== a.Id)]
    }
  }
  // Roots first, so a variant that comes earlier in the list still finds its group.
  for (const a of albumArtists) add(a, true)
  for (const a of everyone) add(a, false)
  return [...groups.values()]
}

/** Ids of the group `id` belongs to (just `[id]` when it is alone or unknown). */
export function groupIdsOf(groups: readonly ArtistGroup[], id: string): string[] {
  return groups.find((g) => g.ids.includes(id))?.ids ?? [id]
}
