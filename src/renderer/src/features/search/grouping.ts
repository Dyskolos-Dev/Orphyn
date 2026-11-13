import type { BaseItemDto, SearchResults } from '@renderer/jellyfin'

export type ResultKind = 'movie' | 'series' | 'episode' | 'artist' | 'album' | 'song'
export type SectionId = 'movies' | 'series' | 'episodes' | 'artists' | 'albums' | 'songs'

export interface ResultSection {
  id: SectionId
  kind: ResultKind
  items: BaseItemDto[]
}

export interface TopResult {
  kind: ResultKind
  item: BaseItemDto
}

export interface GroupedResults {
  top: TopResult | null
  /** Non-empty sections in display order (Movies, Series, Episodes, Artists, Albums, Songs). */
  sections: ResultSection[]
  total: number
}

const SECTIONS: readonly { id: SectionId; kind: ResultKind; key: keyof SearchResults }[] = [
  { id: 'movies', kind: 'movie', key: 'movies' },
  { id: 'series', kind: 'series', key: 'series' },
  { id: 'episodes', kind: 'episode', key: 'episodes' },
  { id: 'artists', kind: 'artist', key: 'artists' },
  { id: 'albums', kind: 'album', key: 'albums' },
  { id: 'songs', kind: 'song', key: 'tracks' },
]

/** When two candidates match equally well, the earlier kind wins the "Top Result". */
const TOP_PRIORITY: readonly ResultKind[] = [
  'movie',
  'series',
  'artist',
  'album',
  'song',
  'episode',
]

const strip = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().trim()

/** 3 = exact name, 2 = name starts with the term, 1 = a word starts with it, 0 = contains it. */
export function matchScore(name: string | null | undefined, term: string): number {
  const n = strip(name ?? '')
  const q = strip(term)
  if (!n || !q) return -1
  if (n === q) return 3
  if (n.startsWith(q)) return 2
  if (n.split(/[\s\-–:._]+/).some((w) => w.startsWith(q))) return 1
  return n.includes(q) ? 0 : -1
}

/** Groups a search response into ordered shelves and picks the best match as the top result. */
export function groupResults(results: SearchResults | undefined, term: string): GroupedResults {
  const sections: ResultSection[] = []
  let top: (TopResult & { score: number; rank: number }) | null = null
  let total = 0
  for (const def of SECTIONS) {
    const items = results?.[def.key] ?? []
    if (items.length === 0) continue
    sections.push({ id: def.id, kind: def.kind, items })
    total += items.length
    for (const item of items) {
      const score = matchScore(item.Name, term)
      const rank = TOP_PRIORITY.indexOf(def.kind)
      if (!top || score > top.score || (score === top.score && rank < top.rank)) {
        top = { kind: def.kind, item, score, rank }
      }
    }
  }
  return { top: top ? { kind: top.kind, item: top.item } : null, sections, total }
}

/** Documented app route for a result (docs/ARCHITECTURE.md). A song opens its album for now. */
export function routeFor(kind: ResultKind, item: BaseItemDto): string {
  const id = item.Id ?? ''
  switch (kind) {
    case 'movie':
    case 'series':
    case 'episode':
      return `/video/item/${id}`
    case 'artist':
      return `/music/artist/${id}`
    case 'album':
      return `/music/album/${id}`
    case 'song':
      return `/music/album/${item.AlbumId ?? id}`
  }
}
