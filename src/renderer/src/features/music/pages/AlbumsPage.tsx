import { Disc3 } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useAlbums } from '@renderer/jellyfin'
import { AlbumCard } from '../components/Cards'
import { LibraryPage, useSortPref, type SortOption } from './LibraryPage'

export function AlbumsPage() {
  const { t } = useTranslation('music')
  const options = useMemo<SortOption[]>(
    () => [
      { id: 'name', label: t('sort.name'), sortBy: ['SortName'], defaultOrder: 'Ascending' },
      {
        id: 'artist',
        label: t('sort.artist'),
        sortBy: ['AlbumArtist', 'SortName'],
        defaultOrder: 'Ascending',
      },
      {
        id: 'year',
        label: t('sort.year'),
        sortBy: ['ProductionYear', 'SortName'],
        defaultOrder: 'Descending',
      },
      {
        id: 'added',
        label: t('sort.added'),
        sortBy: ['DateCreated', 'SortName'],
        defaultOrder: 'Descending',
      },
    ],
    [t],
  )
  const sort = useSortPref('albums', options)
  const query = useAlbums({ sortBy: sort.sortBy, sortOrder: sort.sortOrder, pageSize: 60 })
  return (
    <LibraryPage
      title={t('nav.albums')}
      query={query}
      kind="square"
      render={(album) => <AlbumCard album={album} />}
      emptyIcon={<Disc3 />}
      emptyTitle={t('albums.empty')}
      emptyMessage={t('albums.emptyHint')}
      sortItems={sort.items}
    />
  )
}
