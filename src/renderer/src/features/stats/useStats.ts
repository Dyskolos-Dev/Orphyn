import { useQuery, type QueryClient } from '@tanstack/react-query'
import { useMemo } from 'react'
import { requireApi, useSession } from '@renderer/jellyfin'
import { jfGet } from '@renderer/jellyfin/client'
import * as ep from '@renderer/jellyfin/endpoints'
import type { BaseItemDto } from '@renderer/jellyfin/types'
import {
  aggregateYear,
  availableYears,
  countPluginRows,
  topSongs,
  type YearStats,
} from './aggregate'

const FIELDS = ['PrimaryImageAspectRatio', 'Genres', 'RunTimeTicks', 'ProductionYear', 'UserData']
const PAGE = 500
/** Safety cap per kind: 4000 most recently played items. */
const MAX_PER_KIND = 4000

async function playedOfType(
  type: 'Audio' | 'Movie' | 'Episode',
  userId: string,
  signal?: AbortSignal,
): Promise<BaseItemDto[]> {
  const api = requireApi()
  const out: BaseItemDto[] = []
  for (let start = 0; start < MAX_PER_KIND; start += PAGE) {
    const page = await ep.fetchItemsPage(
      api,
      userId,
      {
        includeItemTypes: [type],
        filters: ['IsPlayed'],
        sortBy: ['DatePlayed'],
        sortOrder: 'Descending',
        fields: FIELDS,
        pageSize: PAGE,
      },
      start,
      signal,
    )
    out.push(...page.items)
    if (start + page.items.length >= page.total || page.items.length === 0) break
  }
  return out
}

type PlayedKind = 'Audio' | 'Movie' | 'Episode'

const playedKey = (accountId: string, userId: string, kinds: readonly PlayedKind[]) =>
  ['jf', accountId, 'stats', 'played', userId, ...(kinds.length === 3 ? [] : kinds)] as const

async function playedOfKinds(
  kinds: readonly PlayedKind[],
  userId: string,
  signal?: AbortSignal,
): Promise<BaseItemDto[]> {
  const lists = await Promise.all(kinds.map((t) => playedOfType(t, userId, signal)))
  return lists.flat()
}

const ALL_KINDS: readonly PlayedKind[] = ['Audio', 'Movie', 'Episode']

/** Every played song, film and episode of the current user. Cached: the page reopens instantly. */
export function usePlayedItems() {
  const account = useSession((s) => s.account)
  const api = useSession((s) => s.api)
  return useQuery({
    queryKey: playedKey(account?.id ?? '', account?.userId ?? '', ALL_KINDS),
    queryFn: ({ signal }) => playedOfKinds(ALL_KINDS, account?.userId ?? '', signal),
    enabled: !!account && !!api,
    staleTime: 10 * 60_000,
    refetchOnWindowFocus: false,
  })
}

const isoDay = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`

/**
 * Exact plays per item from the Playback Reporting plugin, or null when the plugin is not
 * installed (or answers something unexpected): the page then approximates.
 */
async function fetchPluginPlays(
  userId: string,
  year: number,
  signal?: AbortSignal,
): Promise<Map<string, number> | null> {
  try {
    const now = new Date()
    const end = year === now.getFullYear() ? now : new Date(year, 11, 31)
    const days = Math.ceil((end.getTime() - new Date(year, 0, 1).getTime()) / 86_400_000) + 1
    const rows = await jfGet<unknown>(
      requireApi(),
      '/user_usage_stats/UserPlaylist',
      { user_id: userId, aggregate_data: false, days, end_date: isoDay(end) },
      signal,
    )
    return countPluginRows(rows)
  } catch {
    return null
  }
}

const pluginKey = (accountId: string, userId: string, year: number) =>
  ['jf', accountId, 'stats', 'plugin', userId, year] as const

export function usePluginPlays(year: number) {
  const account = useSession((s) => s.account)
  const api = useSession((s) => s.api)
  return useQuery({
    queryKey: pluginKey(account?.id ?? '', account?.userId ?? '', year),
    queryFn: ({ signal }) => fetchPluginPlays(account?.userId ?? '', year, signal),
    enabled: !!account && !!api,
    staleTime: 30 * 60_000,
    refetchOnWindowFocus: false,
    retry: false,
  })
}

/**
 * The most played songs of `year` (up to 50), loaded through the query cache so the stats page
 * and the mixes share one fetch of the played songs.
 */
export async function loadTopSongs(
  qc: QueryClient,
  accountId: string,
  userId: string,
  year: number,
): Promise<BaseItemDto[]> {
  const [items, pluginPlays] = await Promise.all([
    qc.ensureQueryData({
      queryKey: playedKey(accountId, userId, ['Audio']),
      queryFn: ({ signal }) => playedOfKinds(['Audio'], userId, signal),
      staleTime: 10 * 60_000,
    }),
    qc.ensureQueryData({
      queryKey: pluginKey(accountId, userId, year),
      queryFn: ({ signal }) => fetchPluginPlays(userId, year, signal),
      staleTime: 30 * 60_000,
    }),
  ])
  return topSongs(items, year, { pluginPlays: pluginPlays ?? undefined })
}

export interface YearInReview {
  isLoading: boolean
  isError: boolean
  years: number[]
  stats: YearStats | null
  /** The year's most played songs (up to 50), ready to play. */
  topTracks: BaseItemDto[]
  /** True when plays come from the Playback Reporting plugin rather than an approximation. */
  exact: boolean
}

export function useYearInReview(year: number): YearInReview {
  const played = usePlayedItems()
  const plugin = usePluginPlays(year)
  const items = played.data
  const pluginPlays = plugin.data ?? undefined
  const years = useMemo(() => availableYears(items ?? []), [items])
  const stats = useMemo(
    () => (items && !plugin.isLoading ? aggregateYear(items, year, { pluginPlays }) : null),
    [items, year, pluginPlays, plugin.isLoading],
  )
  const topTracks = useMemo(
    () => (items && !plugin.isLoading ? topSongs(items, year, { pluginPlays }) : []),
    [items, year, pluginPlays, plugin.isLoading],
  )
  return {
    isLoading: played.isLoading || (!!items && plugin.isLoading),
    isError: played.isError,
    years,
    stats,
    topTracks,
    exact: !!pluginPlays,
  }
}
