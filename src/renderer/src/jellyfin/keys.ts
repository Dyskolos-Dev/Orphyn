/**
 * Query keys. Every key starts with ['jf', accountId, ...] so data of two servers can never be
 * mixed, and `jfKeys.all(accountId)` invalidates everything for one server.
 */
import type { ItemsParams, ListKind } from './types'

export const jfKeys = {
  all: (acc: string) => ['jf', acc] as const,
  views: (acc: string) => ['jf', acc, 'views'] as const,
  items: (acc: string, params: ItemsParams) => ['jf', acc, 'items', params] as const,
  list: (acc: string, kind: ListKind, params: unknown) =>
    ['jf', acc, 'list', kind, params] as const,
  item: (acc: string, id: string) => ['jf', acc, 'item', id] as const,
  resume: (acc: string, kind: 'video' | 'audio') => ['jf', acc, 'resume', kind] as const,
  nextUp: (acc: string, parentId?: string) => ['jf', acc, 'nextUp', parentId ?? null] as const,
  latest: (acc: string, parentId: string) => ['jf', acc, 'latest', parentId] as const,
  seasons: (acc: string, seriesId: string) => ['jf', acc, 'seasons', seriesId] as const,
  episodes: (acc: string, seriesId: string, seasonId?: string) =>
    ['jf', acc, 'episodes', seriesId, seasonId ?? null] as const,
  similar: (acc: string, id: string) => ['jf', acc, 'similar', id] as const,
  artistAlbums: (acc: string, artistId: string) => ['jf', acc, 'artistAlbums', artistId] as const,
  albumTracks: (acc: string, albumId: string) => ['jf', acc, 'albumTracks', albumId] as const,
  playlistItems: (acc: string, playlistId: string) =>
    ['jf', acc, 'playlistItems', playlistId] as const,
  instantMix: (acc: string, id: string) => ['jf', acc, 'instantMix', id] as const,
  lyrics: (acc: string, id: string) => ['jf', acc, 'lyrics', id] as const,
  search: (acc: string, term: string) => ['jf', acc, 'search', term] as const,
}

/** Stale times (ms): libraries change rarely, resume/next-up must feel fresh. */
export const STALE = {
  views: 10 * 60_000,
  catalog: 5 * 60_000,
  item: 2 * 60_000,
  progress: 15_000,
  lyrics: 60 * 60_000,
  search: 60_000,
} as const
