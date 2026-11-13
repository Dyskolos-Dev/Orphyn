import type { BaseItemDto } from '@renderer/jellyfin'

export type FavoriteTab = 'songs' | 'albums' | 'artists'
export type FavoriteSort = 'name' | 'artist' | 'album' | 'year' | 'added'

/** Sort keys offered per tab. */
export const SORTS: Record<FavoriteTab, readonly FavoriteSort[]> = {
  songs: ['name', 'artist', 'album', 'added'],
  albums: ['name', 'artist', 'year', 'added'],
  artists: ['name', 'added'],
}

const text = (v: string | null | undefined) => v ?? ''
const cmp = (a: string, b: string) =>
  a.localeCompare(b, undefined, { sensitivity: 'base', numeric: true })

const keyOf: Record<FavoriteSort, (i: BaseItemDto) => string | number> = {
  name: (i) => text(i.SortName ?? i.Name),
  artist: (i) => text(i.AlbumArtist ?? i.Artists?.[0]),
  album: (i) => text(i.Album),
  year: (i) => i.ProductionYear ?? 0,
  added: (i) => text(i.DateCreated),
}

/** Sorted copy; ties fall back to the name so the order is stable. */
export function sortFavorites(
  items: readonly BaseItemDto[],
  sort: FavoriteSort,
  order: 'Ascending' | 'Descending',
): BaseItemDto[] {
  const k = keyOf[sort]
  const dir = order === 'Ascending' ? 1 : -1
  return [...items].sort((a, b) => {
    const x = k(a)
    const y = k(b)
    const r = typeof x === 'number' && typeof y === 'number' ? x - y : cmp(String(x), String(y))
    return r !== 0 ? dir * r : cmp(text(a.Name), text(b.Name))
  })
}
