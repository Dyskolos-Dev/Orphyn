/** Typed fetchers (one per Jellyfin endpoint). Hooks wrap these; tests call them directly. */
import type { Api } from '@jellyfin/sdk'
import { jfGet, jfRequest, type QueryParams } from './client'
import type {
  BaseItemDto,
  ItemsPage,
  ItemsParams,
  ListKind,
  ListParams,
  Lyrics,
  SearchResults,
  TimedLyricLine,
} from './types'

interface QueryResult {
  Items?: BaseItemDto[]
  TotalRecordCount?: number
  StartIndex?: number
}

export const DEFAULT_FIELDS = [
  'PrimaryImageAspectRatio',
  'Overview',
  'Genres',
  'ProductionYear',
  'RunTimeTicks',
  'ParentId',
  'DateCreated',
]

export const DEFAULT_PAGE_SIZE = 50

const toPage = (r: QueryResult, startIndex: number): ItemsPage => ({
  items: r.Items ?? [],
  total: r.TotalRecordCount ?? r.Items?.length ?? 0,
  startIndex: r.StartIndex ?? startIndex,
})

const listParams = (p: ListParams, startIndex: number): QueryParams => ({
  parentId: p.parentId,
  searchTerm: p.searchTerm,
  sortBy: p.sortBy,
  sortOrder: p.sortOrder,
  isFavorite: p.isFavorite,
  genres: p.genres,
  startIndex,
  limit: p.pageSize ?? DEFAULT_PAGE_SIZE,
  enableTotalRecordCount: true,
  fields: DEFAULT_FIELDS,
})

/** Genres of the video (`Movie` + `Series`) or music library, by name. */
export async function fetchGenres(
  api: Api,
  userId: string,
  kind: 'video' | 'music',
): Promise<BaseItemDto[]> {
  const r = await jfGet<QueryResult>(api, kind === 'music' ? '/MusicGenres' : '/Genres', {
    userId,
    ...(kind === 'video' ? { includeItemTypes: ['Movie', 'Series'] } : {}),
    sortBy: ['SortName'],
    sortOrder: 'Ascending',
    enableTotalRecordCount: false,
  })
  return r.Items ?? []
}

export async function fetchUserViews(api: Api, userId: string): Promise<BaseItemDto[]> {
  const r = await jfGet<QueryResult>(api, '/UserViews', { userId })
  return r.Items ?? []
}

export async function fetchItemsPage(
  api: Api,
  userId: string,
  p: ItemsParams,
  startIndex: number,
  signal?: AbortSignal,
): Promise<ItemsPage> {
  const r = await jfGet<QueryResult>(
    api,
    '/Items',
    {
      ...listParams(p, startIndex),
      userId,
      recursive: p.recursive ?? true,
      includeItemTypes: p.includeItemTypes,
      filters: p.filters,
      artistIds: p.artistIds,
      albumArtistIds: p.albumArtistIds,
      years: p.years,
      mediaTypes: p.mediaTypes,
      ...(p.releasedOnly ? { maxPremiereDate: new Date().toISOString() } : {}),
      ...(p.fields ? { fields: p.fields } : {}),
    },
    signal,
  )
  return toPage(r, startIndex)
}

/** Paginated music/playlist lists. Artists use their own endpoints, the rest are GET /Items. */
export async function fetchListPage(
  api: Api,
  userId: string,
  kind: ListKind,
  p: ListParams,
  startIndex: number,
  signal?: AbortSignal,
): Promise<ItemsPage> {
  if (kind === 'artists' || kind === 'albumArtists') {
    const path = kind === 'artists' ? '/Artists' : '/Artists/AlbumArtists'
    const r = await jfGet<QueryResult>(api, path, { ...listParams(p, startIndex), userId }, signal)
    return toPage(r, startIndex)
  }
  const types = { albums: 'MusicAlbum', playlists: 'Playlist', songs: 'Audio' } as const
  return fetchItemsPage(
    api,
    userId,
    {
      ...p,
      includeItemTypes: [types[kind]],
      sortBy: p.sortBy ?? (kind === 'songs' ? ['Album', 'SortName'] : ['SortName']),
      sortOrder: p.sortOrder ?? 'Ascending',
    },
    startIndex,
    signal,
  )
}

export function fetchItem(api: Api, userId: string, id: string): Promise<BaseItemDto> {
  return jfGet<BaseItemDto>(api, `/Users/${userId}/Items/${id}`)
}

export async function fetchResume(
  api: Api,
  userId: string,
  kind: 'video' | 'audio',
  limit = 20,
): Promise<BaseItemDto[]> {
  const r = await jfGet<QueryResult>(api, '/UserItems/Resume', {
    userId,
    mediaTypes: kind === 'video' ? 'Video' : 'Audio',
    limit,
    fields: DEFAULT_FIELDS,
    enableTotalRecordCount: false,
  })
  return r.Items ?? []
}

export async function fetchNextUp(
  api: Api,
  userId: string,
  parentId?: string,
  limit = 20,
): Promise<BaseItemDto[]> {
  const r = await jfGet<QueryResult>(api, '/Shows/NextUp', {
    userId,
    parentId,
    limit,
    fields: DEFAULT_FIELDS,
    enableTotalRecordCount: false,
  })
  return r.Items ?? []
}

export function fetchLatest(
  api: Api,
  userId: string,
  parentId: string,
  limit = 20,
): Promise<BaseItemDto[]> {
  return jfGet<BaseItemDto[]>(api, '/Items/Latest', {
    userId,
    parentId,
    limit,
    fields: DEFAULT_FIELDS,
  })
}

export async function fetchSeasons(api: Api, userId: string, seriesId: string) {
  const r = await jfGet<QueryResult>(api, `/Shows/${seriesId}/Seasons`, {
    userId,
    fields: DEFAULT_FIELDS,
  })
  return r.Items ?? []
}

export async function fetchEpisodes(api: Api, userId: string, seriesId: string, seasonId?: string) {
  const r = await jfGet<QueryResult>(api, `/Shows/${seriesId}/Episodes`, {
    userId,
    seasonId,
    fields: [...DEFAULT_FIELDS, 'MediaSources'],
  })
  return r.Items ?? []
}

export async function fetchSimilar(api: Api, userId: string, id: string, limit = 12) {
  const r = await jfGet<QueryResult>(api, `/Items/${id}/Similar`, {
    userId,
    limit,
    fields: DEFAULT_FIELDS,
  })
  return r.Items ?? []
}

/** GET /Items/{id}/LocalTrailers: trailer files stored next to the item on the server. */
export async function fetchLocalTrailers(
  api: Api,
  userId: string,
  id: string,
): Promise<BaseItemDto[]> {
  const r = await jfRequest<BaseItemDto[] | undefined>(api, 'GET', `/Items/${id}/LocalTrailers`, {
    params: { userId },
    allowStatus: [404],
  })
  return Array.isArray(r) ? r : []
}

export async function fetchArtistAlbums(api: Api, userId: string, artistId: string) {
  const page = await fetchItemsPage(
    api,
    userId,
    {
      includeItemTypes: ['MusicAlbum'],
      artistIds: [artistId],
      sortBy: ['ProductionYear', 'SortName'],
      sortOrder: 'Descending',
      pageSize: 500,
    },
    0,
  )
  return page.items
}

export async function fetchAlbumTracks(api: Api, userId: string, albumId: string) {
  const page = await fetchItemsPage(
    api,
    userId,
    {
      parentId: albumId,
      includeItemTypes: ['Audio'],
      sortBy: ['ParentIndexNumber', 'IndexNumber', 'SortName'],
      sortOrder: 'Ascending',
      pageSize: 1000,
      fields: [...DEFAULT_FIELDS, 'MediaSources'],
    },
    0,
  )
  return page.items
}

export async function fetchPlaylistItems(api: Api, userId: string, playlistId: string) {
  const r = await jfGet<QueryResult>(api, `/Playlists/${playlistId}/Items`, {
    userId,
    fields: [...DEFAULT_FIELDS, 'MediaSources'],
  })
  return r.Items ?? []
}

export async function fetchInstantMix(api: Api, userId: string, id: string, limit = 50) {
  const r = await jfGet<QueryResult>(api, `/Items/${id}/InstantMix`, {
    userId,
    limit,
    fields: DEFAULT_FIELDS,
  })
  return r.Items ?? []
}

interface LyricDto {
  Lyrics?: { Text?: string; Start?: number | null }[]
}

const TICKS_PER_MS = 10_000

/** Jellyfin ≥ 10.9 lyrics: `null` when the track has none (404). Start ticks are converted to ms. */
export async function fetchLyrics(api: Api, itemId: string): Promise<Lyrics | null> {
  const dto = await jfRequest<LyricDto | undefined>(api, 'GET', `/Audio/${itemId}/Lyrics`, {
    allowStatus: [404],
  })
  const raw = dto?.Lyrics
  if (!raw || raw.length === 0) return null
  const lines: TimedLyricLine[] = raw.map((l) => ({
    text: l.Text ?? '',
    startMs: typeof l.Start === 'number' ? Math.round(l.Start / TICKS_PER_MS) : null,
  }))
  return { synced: lines.every((l) => l.startMs !== null), lines }
}

/** Grouped search: one small request per type so each group is relevant and bounded. */
export async function fetchSearch(
  api: Api,
  userId: string,
  term: string,
  limitPerType = 12,
  signal?: AbortSignal,
): Promise<SearchResults> {
  const byType = (type: string) =>
    fetchItemsPage(
      api,
      userId,
      { searchTerm: term, includeItemTypes: [type], pageSize: limitPerType },
      0,
      signal,
    ).then((p) => p.items)
  const [movies, series, episodes, albums, tracks, artists] = await Promise.all([
    byType('Movie'),
    byType('Series'),
    byType('Episode'),
    byType('MusicAlbum'),
    byType('Audio'),
    fetchListPage(
      api,
      userId,
      'artists',
      { searchTerm: term, pageSize: limitPerType },
      0,
      signal,
    ).then((p) => p.items),
  ])
  return { movies, series, episodes, albums, artists, tracks }
}

export const setPlayed = (api: Api, userId: string, id: string, played: boolean) =>
  jfRequest<unknown>(api, played ? 'POST' : 'DELETE', `/UserPlayedItems/${id}`, {
    params: { userId },
  })

export const setFavorite = (api: Api, userId: string, id: string, favorite: boolean) =>
  jfRequest<unknown>(api, favorite ? 'POST' : 'DELETE', `/UserFavoriteItems/${id}`, {
    params: { userId },
  })

// ---------------------------------------------------------------- playlists

/** POST /Playlists. `itemIds` may hold songs or albums (the server expands albums). Returns the new id. */
export async function createPlaylist(
  api: Api,
  userId: string,
  name: string,
  itemIds: readonly string[] = [],
  opts: { isPublic?: boolean } = {},
): Promise<string> {
  const r = await jfRequest<{ Id?: string }>(api, 'POST', '/Playlists', {
    data: {
      Name: name,
      Ids: [...itemIds],
      UserId: userId,
      MediaType: 'Audio',
      IsPublic: opts.isPublic ?? false,
    },
  })
  if (!r?.Id) throw new Error('Jellyfin did not return the new playlist id')
  return r.Id
}

/** POST /Playlists/{id} (Jellyfin ≥ 10.9). */
export const renamePlaylist = (api: Api, playlistId: string, name: string) =>
  jfRequest<unknown>(api, 'POST', `/Playlists/${playlistId}`, { data: { Name: name } })

/** A playlist is a library item: DELETE /Items/{id}. */
export const deletePlaylist = (api: Api, playlistId: string) =>
  jfRequest<unknown>(api, 'DELETE', `/Items/${playlistId}`)

/** Appends songs (or albums) to the end of a playlist. */
export const addToPlaylist = (
  api: Api,
  userId: string,
  playlistId: string,
  itemIds: readonly string[],
) =>
  jfRequest<unknown>(api, 'POST', `/Playlists/${playlistId}/Items`, {
    params: { ids: itemIds, userId },
  })

/** `entryIds` are PlaylistItemIds (one per occurrence), not track ids. */
export const removeFromPlaylist = (api: Api, playlistId: string, entryIds: readonly string[]) =>
  jfRequest<unknown>(api, 'DELETE', `/Playlists/${playlistId}/Items`, {
    params: { entryIds },
  })

/** Moves one entry (PlaylistItemId) to a 0-based position. */
export const movePlaylistItem = (api: Api, playlistId: string, entryId: string, newIndex: number) =>
  jfRequest<unknown>(api, 'POST', `/Playlists/${playlistId}/Items/${entryId}/Move/${newIndex}`)

/** True when the user may edit the playlist (i.e. owns it or was granted edit rights). */
export async function canEditPlaylist(
  api: Api,
  userId: string,
  playlistId: string,
): Promise<boolean> {
  try {
    const r = await jfRequest<{ CanEdit?: boolean } | undefined>(
      api,
      'GET',
      `/Playlists/${playlistId}/Users/${userId}`,
      { allowStatus: [404] },
    )
    return !!r?.CanEdit
  } catch {
    return false
  }
}

/** The user's editable music playlists, by name. */
export async function fetchUserPlaylists(api: Api, userId: string): Promise<BaseItemDto[]> {
  const page = await fetchItemsPage(
    api,
    userId,
    {
      includeItemTypes: ['Playlist'],
      sortBy: ['SortName'],
      sortOrder: 'Ascending',
      pageSize: 500,
      fields: [...DEFAULT_FIELDS, 'ChildCount'],
    },
    0,
  )
  const audio = page.items.filter((p) => !p.MediaType || p.MediaType === 'Audio')
  const editable = await Promise.all(audio.map((p) => canEditPlaylist(api, userId, p.Id ?? '')))
  return audio.filter((_, i) => editable[i])
}

/** Favourite songs / albums / artists, fully loaded (up to 2000 each). */
export async function fetchFavorites(
  api: Api,
  userId: string,
  kind: 'songs' | 'albums' | 'artists',
): Promise<BaseItemDto[]> {
  const page = await fetchListPage(
    api,
    userId,
    kind,
    { isFavorite: true, pageSize: 2000, sortBy: kind === 'songs' ? ['SortName'] : undefined },
    0,
  )
  return page.items
}

/**
 * Best effort: Jellyfin only lets administrators edit item metadata (POST /Items/{id}), so a plain
 * user's description is silently not saved. Resolves with whether it was.
 */
export async function setPlaylistDescription(
  api: Api,
  userId: string,
  playlistId: string,
  description: string,
): Promise<boolean> {
  try {
    const item = await fetchItem(api, userId, playlistId)
    await jfRequest<unknown>(api, 'POST', `/Items/${playlistId}`, {
      data: { ...item, Overview: description },
    })
    return true
  } catch {
    return false
  }
}
