import { ListMusic, Plus } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@renderer/design'
import { useItems } from '@renderer/jellyfin'
import { PlaylistCard } from '../components/Cards'
import { openNewPlaylist } from '../playlists/ui'
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
  const query = useItems({
    includeItemTypes: ['Playlist'],
    mediaTypes: ['Audio'],
    sortBy: sort.sortBy,
    sortOrder: sort.sortOrder,
    pageSize: 60,
  })
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
      actions={
        <Button icon={<Plus />} onClick={() => openNewPlaylist()}>
          {t('playlistSheet.title')}
        </Button>
      }
    />
  )
}
