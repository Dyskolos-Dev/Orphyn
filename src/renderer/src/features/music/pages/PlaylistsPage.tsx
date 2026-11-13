import { ListMusic } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { usePlaylists } from '@renderer/jellyfin'
import { PlaylistCard } from '../components/Cards'
import { LibraryPage, useSortPref, type SortOption } from './LibraryPage'

export function PlaylistsPage() {
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
  const sort = useSortPref('playlists', options)
  const query = usePlaylists({ sortBy: sort.sortBy, sortOrder: sort.sortOrder, pageSize: 60 })
  return (
    <LibraryPage
      title={t('nav.playlists')}
      query={query}
      kind="square"
      render={(playlist) => <PlaylistCard playlist={playlist} />}
      emptyIcon={<ListMusic />}
      emptyTitle={t('playlists.empty')}
      emptyMessage={t('playlists.emptyHint')}
      sortItems={sort.items}
    />
  )
}
