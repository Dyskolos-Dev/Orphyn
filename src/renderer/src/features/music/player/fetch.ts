/**
 * Imperative fetches for "play this album / playlist / artist" actions triggered outside React
 * (search results, cast, context menus). They reuse the data layer's endpoint functions and the
 * shared query cache, so a page that already loaded the tracks costs no request.
 */
import { queryClient, requireApi, useSession, jfKeys } from '@renderer/jellyfin'
import * as ep from '@renderer/jellyfin/endpoints'
import type { Track } from './types'

function ctx() {
  const api = requireApi()
  const account = useSession.getState().account
  if (!account) throw new Error('No active account')
  return { api, userId: account.userId, acc: account.id }
}

export async function fetchAlbumTracks(albumId: string): Promise<Track[]> {
  const { api, userId, acc } = ctx()
  return queryClient.fetchQuery({
    queryKey: jfKeys.albumTracks(acc, albumId),
    queryFn: () => ep.fetchAlbumTracks(api, userId, albumId),
    staleTime: 5 * 60_000,
  })
}

export async function fetchPlaylistTracks(playlistId: string): Promise<Track[]> {
  const { api, userId, acc } = ctx()
  return queryClient.fetchQuery({
    queryKey: jfKeys.playlistItems(acc, playlistId),
    queryFn: () => ep.fetchPlaylistItems(api, userId, playlistId),
    staleTime: 60_000,
  })
}

export async function fetchItem(id: string): Promise<Track> {
  const { api, userId, acc } = ctx()
  return queryClient.fetchQuery({
    queryKey: jfKeys.item(acc, id),
    queryFn: () => ep.fetchItem(api, userId, id),
    staleTime: 2 * 60_000,
  })
}

/** Tracks of a song, album, playlist or artist, in playing order. */
export async function fetchTracksOf(id: string): Promise<Track[]> {
  const item = await fetchItem(id)
  if (item.Type === 'MusicAlbum') return fetchAlbumTracks(id)
  if (item.Type === 'Playlist') return fetchPlaylistTracks(id)
  if (item.Type === 'MusicArtist') return fetchArtistSongs(id)
  return [item]
}

export async function fetchInstantMix(id: string, limit = 50): Promise<Track[]> {
  const { api, userId } = ctx()
  return ep.fetchInstantMix(api, userId, id, limit)
}

/** Every song of an artist (for the artist page's Play / Shuffle). */
export async function fetchArtistSongs(artistId: string, limit = 500): Promise<Track[]> {
  const { api, userId } = ctx()
  const page = await ep.fetchItemsPage(
    api,
    userId,
    {
      includeItemTypes: ['Audio'],
      artistIds: [artistId],
      sortBy: ['Album', 'ParentIndexNumber', 'IndexNumber'],
      sortOrder: 'Ascending',
      pageSize: limit,
    },
    0,
  )
  return page.items
}
