/** TanStack Query hooks: the only way screens read Jellyfin data. All are scoped to the active server. */
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
  type InfiniteData,
} from '@tanstack/react-query'
import type { Api } from '@jellyfin/sdk'
import * as ep from './endpoints'
import { STALE, jfKeys } from './keys'
import { useSession } from './session'
import type { BaseItemDto, ItemsPage, ItemsParams, ListKind, ListParams } from './types'

interface Ctx {
  api: Api
  userId: string
  acc: string
}

/** Active server context, or null while no server is active / login is required. */
function useCtx(): Ctx | null {
  const api = useSession((s) => s.api)
  const account = useSession((s) => s.account)
  return api && account ? { api, userId: account.userId, acc: account.id } : null
}

/** Builds a query that is disabled until a session exists. */
function useScoped<T>(
  keyFor: (acc: string) => readonly unknown[],
  fetcher: (c: Ctx, signal: AbortSignal) => Promise<T>,
  opts: { staleTime: number; enabled?: boolean },
) {
  const ctx = useCtx()
  return useQuery({
    queryKey: ctx ? keyFor(ctx.acc) : ['jf', 'none'],
    queryFn: ({ signal }) => fetcher(ctx as Ctx, signal),
    enabled: !!ctx && (opts.enabled ?? true),
    staleTime: opts.staleTime,
  })
}

/** Flattens infinite pages into one item array. */
export function flattenPages(data: InfiniteData<ItemsPage> | undefined): BaseItemDto[] {
  return data?.pages.flatMap((p) => p.items) ?? []
}

function nextPageParam(last: ItemsPage): number | undefined {
  const next = last.startIndex + last.items.length
  return last.items.length > 0 && next < last.total ? next : undefined
}

function useInfinitePages(
  keyFor: (acc: string) => readonly unknown[],
  fetchPage: (c: Ctx, start: number, signal: AbortSignal) => Promise<ItemsPage>,
  enabled: boolean | undefined,
) {
  const ctx = useCtx()
  return useInfiniteQuery({
    queryKey: ctx ? keyFor(ctx.acc) : ['jf', 'none'],
    queryFn: ({ pageParam, signal }) => fetchPage(ctx as Ctx, pageParam, signal),
    initialPageParam: 0,
    getNextPageParam: nextPageParam,
    enabled: !!ctx && (enabled ?? true),
    staleTime: STALE.catalog,
  })
}

// ---------------------------------------------------------------- libraries & items

/** The user's libraries (Movies, TV Shows, Music, ...). */
export const useUserViews = () =>
  useScoped(
    (a) => jfKeys.views(a),
    (c) => ep.fetchUserViews(c.api, c.userId),
    {
      staleTime: STALE.views,
    },
  )

/** Generic infinite GET /Items. Use `flattenPages(query.data)` and `query.fetchNextPage()`. */
export const useItems = (params: ItemsParams = {}) =>
  useInfinitePages(
    (a) => jfKeys.items(a, params),
    (c, start, signal) => ep.fetchItemsPage(c.api, c.userId, params, start, signal),
    params.enabled,
  )

export const useItem = (id: string | undefined) =>
  useScoped(
    (a) => jfKeys.item(a, id ?? ''),
    (c) => ep.fetchItem(c.api, c.userId, id as string),
    {
      staleTime: STALE.item,
      enabled: !!id,
    },
  )

/** Continue watching (`video`) / continue listening (`audio`). */
export const useResumeItems = (kind: 'video' | 'audio' = 'video', limit = 20) =>
  useScoped(
    (a) => [...jfKeys.resume(a, kind), limit],
    (c) => ep.fetchResume(c.api, c.userId, kind, limit),
    {
      staleTime: STALE.progress,
    },
  )

export const useNextUp = (parentId?: string, limit = 20) =>
  useScoped(
    (a) => [...jfKeys.nextUp(a, parentId), limit],
    (c) => ep.fetchNextUp(c.api, c.userId, parentId, limit),
    {
      staleTime: STALE.progress,
    },
  )

/** Recently added to a library. */
export const useLatest = (parentId: string | undefined, limit = 20) =>
  useScoped(
    (a) => [...jfKeys.latest(a, parentId ?? ''), limit],
    (c) => ep.fetchLatest(c.api, c.userId, parentId as string, limit),
    { staleTime: STALE.catalog, enabled: !!parentId },
  )

export const useSeasons = (seriesId: string | undefined) =>
  useScoped(
    (a) => jfKeys.seasons(a, seriesId ?? ''),
    (c) => ep.fetchSeasons(c.api, c.userId, seriesId as string),
    {
      staleTime: STALE.catalog,
      enabled: !!seriesId,
    },
  )

/** Episodes of a series, optionally of one season. */
export const useEpisodes = (seriesId: string | undefined, seasonId?: string) =>
  useScoped(
    (a) => jfKeys.episodes(a, seriesId ?? '', seasonId),
    (c) => ep.fetchEpisodes(c.api, c.userId, seriesId as string, seasonId),
    { staleTime: STALE.item, enabled: !!seriesId },
  )

export const useSimilar = (id: string | undefined, limit = 12) =>
  useScoped(
    (a) => [...jfKeys.similar(a, id ?? ''), limit],
    (c) => ep.fetchSimilar(c.api, c.userId, id as string, limit),
    {
      staleTime: STALE.catalog,
      enabled: !!id,
    },
  )

// ---------------------------------------------------------------- music

const useList = (kind: ListKind, params: ListParams) =>
  useInfinitePages(
    (a) => jfKeys.list(a, kind, params),
    (c, start, signal) => ep.fetchListPage(c.api, c.userId, kind, params, start, signal),
    params.enabled,
  )

export const useAlbums = (params: ListParams = {}) => useList('albums', params)
export const useArtists = (params: ListParams = {}) => useList('artists', params)
export const useAlbumArtists = (params: ListParams = {}) => useList('albumArtists', params)
export const usePlaylists = (params: ListParams = {}) => useList('playlists', params)
export const useSongs = (params: ListParams = {}) => useList('songs', params)

export const useArtistAlbums = (artistId: string | undefined) =>
  useScoped(
    (a) => jfKeys.artistAlbums(a, artistId ?? ''),
    (c) => ep.fetchArtistAlbums(c.api, c.userId, artistId as string),
    { staleTime: STALE.catalog, enabled: !!artistId },
  )

/** Tracks of an album in disc/track order. */
export const useAlbumTracks = (albumId: string | undefined) =>
  useScoped(
    (a) => jfKeys.albumTracks(a, albumId ?? ''),
    (c) => ep.fetchAlbumTracks(c.api, c.userId, albumId as string),
    { staleTime: STALE.catalog, enabled: !!albumId },
  )

/** Playlist entries in playlist order. */
export const usePlaylistItems = (playlistId: string | undefined) =>
  useScoped(
    (a) => jfKeys.playlistItems(a, playlistId ?? ''),
    (c) => ep.fetchPlaylistItems(c.api, c.userId, playlistId as string),
    { staleTime: STALE.item, enabled: !!playlistId },
  )

/** Instant mix seeded by a song, album, artist, playlist or genre. */
export const useInstantMix = (itemId: string | undefined, limit = 50) =>
  useScoped(
    (a) => [...jfKeys.instantMix(a, itemId ?? ''), limit],
    (c) => ep.fetchInstantMix(c.api, c.userId, itemId as string, limit),
    { staleTime: STALE.item, enabled: !!itemId },
  )

/**
 * Lyrics of a track: `null` when it has none; `synced` tells whether lines carry timestamps.
 * Pass the track's `HasLyrics` (Jellyfin ≥ 10.9 fills it in item lists): when it is `false` no
 * request is made, so tracks without lyrics do not cost a 404 each.
 */
export const useLyrics = (itemId: string | undefined, hasLyrics?: boolean | null) =>
  useScoped(
    (a) => jfKeys.lyrics(a, itemId ?? ''),
    (c) => ep.fetchLyrics(c.api, itemId as string),
    {
      staleTime: STALE.lyrics,
      enabled: !!itemId && hasLyrics !== false,
    },
  )

// ---------------------------------------------------------------- search

/** Search grouped by type. Disabled for terms shorter than 2 characters. */
export const useSearch = (term: string) => {
  const t = term.trim()
  return useScoped(
    (a) => jfKeys.search(a, t),
    (c, signal) => ep.fetchSearch(c.api, c.userId, t, 12, signal),
    { staleTime: STALE.search, enabled: t.length >= 2 },
  )
}

// ---------------------------------------------------------------- mutations

function useItemMutation(run: (c: Ctx, vars: { id: string; value: boolean }) => Promise<unknown>) {
  const qc = useQueryClient()
  const ctx = useCtx()
  return useMutation({
    mutationFn: (vars: { id: string; value: boolean }) => run(ctx as Ctx, vars),
    onSettled: () => {
      // Item flags show up in many lists (resume, next up, albums...): refetch this server's data.
      if (ctx) void qc.invalidateQueries({ queryKey: jfKeys.all(ctx.acc) })
    },
  })
}

/** `mutate({ id, value: true })` marks played, `false` unplayed. */
export const useMarkPlayed = () =>
  useItemMutation((c, v) => ep.setPlayed(c.api, c.userId, v.id, v.value))

/** `mutate({ id, value: true })` adds to favourites, `false` removes. */
export const useToggleFavorite = () =>
  useItemMutation((c, v) => ep.setFavorite(c.api, c.userId, v.id, v.value))
