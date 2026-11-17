import { User } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useAlbumArtists, useArtistLists, type BaseItemDto } from '@renderer/jellyfin'
import { usePlaybackPreferences } from '@renderer/features/settings/preferences'
import { ArtistCard } from '../components/Cards'
import { groupArtists } from '../library/artistGroups'
import { LibraryPage, useSortPref, type SortOption } from './LibraryPage'

export function ArtistsPage() {
  const { t } = useTranslation('music')
  const options = useMemo<SortOption[]>(
    () => [
      { id: 'name', label: t('sort.name'), sortBy: ['SortName'], defaultOrder: 'Ascending' },
      {
        id: 'added',
        label: t('sort.added'),
        sortBy: ['DateCreated', 'SortName'],
        defaultOrder: 'Descending',
      },
    ],
    [t],
  )
  const sort = useSortPref('artists', options)
  const { groupArtists: grouping } = usePlaybackPreferences()
  const paged = useAlbumArtists({
    sortBy: sort.sortBy,
    sortOrder: sort.sortOrder,
    pageSize: 80,
    enabled: !grouping,
  })
  const lists = useArtistLists(grouping)
  const merged = useMemo(() => {
    if (!grouping || !lists.albumArtists) return undefined
    const items = groupArtists(lists.albumArtists, lists.everyone).map((g) => g.item)
    // The server lists are name-sorted; "recently added" sorts the merged entries client-side.
    return sort.sortBy[0] === 'DateCreated'
      ? byDate(items, sort.sortOrder)
      : sortByName(items, sort.sortOrder)
  }, [grouping, lists.albumArtists, lists.everyone, sort.sortBy, sort.sortOrder])
  const query = grouping
    ? {
        data: merged && {
          pageParams: [0],
          pages: [{ items: merged, total: merged.length, startIndex: 0 }],
        },
        isLoading: lists.isLoading,
        isError: lists.isError,
        hasNextPage: false,
        isFetchingNextPage: false,
        fetchNextPage: () => undefined,
        refetch: lists.refetch,
      }
    : paged
  return (
    <LibraryPage
      title={t('nav.artists')}
      query={query}
      kind="round"
      render={(artist) => <ArtistCard artist={artist} />}
      emptyIcon={<User />}
      emptyTitle={t('artists.empty')}
      emptyMessage={t('artists.emptyHint')}
      sortItems={sort.items}
    />
  )
}

const name = (a: BaseItemDto) => a.SortName ?? a.Name ?? ''

function sortByName(items: BaseItemDto[], order: 'Ascending' | 'Descending') {
  const sorted = [...items].sort((a, b) =>
    name(a).localeCompare(name(b), undefined, { sensitivity: 'base' }),
  )
  return order === 'Descending' ? sorted.reverse() : sorted
}

function byDate(items: BaseItemDto[], order: 'Ascending' | 'Descending') {
  const time = (a: BaseItemDto) => Date.parse(a.DateCreated ?? '') || 0
  const sorted = [...items].sort((a, b) => time(a) - time(b))
  return order === 'Descending' ? sorted.reverse() : sorted
}
