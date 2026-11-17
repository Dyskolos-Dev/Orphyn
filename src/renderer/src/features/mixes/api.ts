/**
 * Jellyfin calls of the mixes and radios. They use the session's api client and the data layer's
 * endpoint functions; nothing here touches src/renderer/src/jellyfin.
 */
import { requireApi, useSession } from '@renderer/jellyfin'
import * as ep from '@renderer/jellyfin/endpoints'
import type { MixSource, Track } from './types'

function ctx() {
  const api = requireApi()
  const account = useSession.getState().account
  if (!account) throw new Error('No active account')
  return { api, userId: account.userId, accountId: account.id }
}

export const activeMixesScope = (): { accountId: string; userId: string } => {
  const { accountId, userId } = ctx()
  return { accountId, userId }
}

const FIELDS = [...ep.DEFAULT_FIELDS, 'Genres']

async function songs(
  params: Parameters<typeof ep.fetchItemsPage>[2],
  limit: number,
): Promise<Track[]> {
  const { api, userId } = ctx()
  const page = await ep.fetchItemsPage(
    api,
    userId,
    { includeItemTypes: ['Audio'], fields: FIELDS, ...params, pageSize: limit },
    0,
  )
  return page.items
}

const settled = async <T>(jobs: Promise<T[]>[]): Promise<T[]> =>
  (await Promise.all(jobs.map((j) => j.catch(() => [] as T[])))).flat()

/** Mix data read from the active server. */
export function createMixSource(): MixSource {
  return {
    async signals() {
      const [played, favourites] = await Promise.all([
        songs({ filters: ['IsPlayed'], sortBy: ['PlayCount'], sortOrder: 'Descending' }, 400),
        songs({ isFavorite: true, sortBy: ['DatePlayed'], sortOrder: 'Ascending' }, 300),
      ])
      return { played, favourites }
    },
    async similarTo(trackId, limit) {
      const { api, userId } = ctx()
      return ep.fetchInstantMix(api, userId, trackId, limit)
    },
    async discoveryPool(artistIds, limit) {
      const { api, userId } = ctx()
      const similar = await settled(
        artistIds.slice(0, 4).map((id) => ep.fetchSimilar(api, userId, id, 6)),
      )
      const close = [...new Set([...similar.map((a) => a.Id ?? ''), ...artistIds])].filter(Boolean)
      return settled(
        close
          .slice(0, 10)
          .map((id) =>
            songs(
              { artistIds: [id], filters: ['IsUnplayed'], sortBy: ['Random'] },
              Math.ceil(limit / 6),
            ),
          ),
      )
    },
    genreTracks: (genre, limit) => songs({ genres: [genre], sortBy: ['Random'] }, limit),
  }
}

/**
 * More songs for a radio seeded by a song, album or artist: its Instant Mix, the Instant Mix of
 * the song being played, and a few songs of similar artists. Not deduplicated (the radio does it).
 */
export async function fetchRadioBatch(seedId: string, currentId?: string): Promise<Track[]> {
  const { api, userId } = ctx()
  const [seedMix, currentMix, similar] = await Promise.all([
    ep.fetchInstantMix(api, userId, seedId, 50).catch(() => [] as Track[]),
    currentId && currentId !== seedId
      ? ep.fetchInstantMix(api, userId, currentId, 30).catch(() => [] as Track[])
      : Promise.resolve([] as Track[]),
    ep
      .fetchSimilar(api, userId, seedId, 6)
      .then((items) =>
        settled(
          items
            .filter((a) => a.Type === 'MusicArtist')
            .slice(0, 4)
            .map((a) => songs({ artistIds: [a.Id ?? ''], sortBy: ['Random'] }, 4)),
        ),
      )
      .catch(() => [] as Track[]),
  ])
  // Interleave so a refill is not "all of one source" in a row.
  const out: Track[] = []
  const lists = [seedMix, currentMix, similar]
  for (let i = 0; lists.some((l) => i < l.length); i++) {
    for (const l of lists) if (l[i]) out.push(l[i] as Track)
  }
  return out
}

/** Creates a playlist with the given songs; returns its id. */
export async function savePlaylist(name: string, trackIds: readonly string[]): Promise<string> {
  const { api, userId } = ctx()
  return ep.createPlaylist(api, userId, name, trackIds)
}
