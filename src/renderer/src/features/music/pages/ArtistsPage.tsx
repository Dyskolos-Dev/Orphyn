import { User } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useAlbumArtists } from '@renderer/jellyfin'
import { ArtistCard } from '../components/Cards'
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
  const query = useAlbumArtists({ sortBy: sort.sortBy, sortOrder: sort.sortOrder, pageSize: 80 })
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
