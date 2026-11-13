import { ArrowUpDown, Heart, Play, Shuffle } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  Button,
  EmptyState,
  Grid,
  MenuButton,
  SegmentedControl,
  Spinner,
  type MenuItem,
} from '@renderer/design'
import { useFavoriteAlbums, useFavoriteArtists, useFavoriteTracks } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { AlbumCard, ArtistCard, GridSkeleton } from '../components/Cards'
import { SongRows } from '../components/TrackRows'
import { musicPlayer } from '../player/api'
import { SORTS, sortFavorites, type FavoriteSort, type FavoriteTab } from './sorting'

const STORAGE_KEY = 'orphyn.music.favorites'

interface Pref {
  tab: FavoriteTab
  sort: FavoriteSort
  order: 'Ascending' | 'Descending'
}

function loadPref(): Pref {
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<Pref> | null
    const tab = raw?.tab && raw.tab in SORTS ? raw.tab : 'songs'
    const sort = raw?.sort && SORTS[tab].includes(raw.sort) ? raw.sort : 'name'
    return { tab, sort, order: raw?.order === 'Descending' ? 'Descending' : 'Ascending' }
  } catch {
    return { tab: 'songs', sort: 'name', order: 'Ascending' }
  }
}

/** Everything the user hearted: songs, albums and artists, sortable. */
export function FavoritesPage() {
  const { t } = useTranslation('music')
  const [pref, setPref] = useState<Pref>(loadPref)
  const update = (next: Pref) => {
    setPref(next)
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
    } catch {
      /* best effort */
    }
  }
  const songs = useFavoriteTracks()
  const albums = useFavoriteAlbums()
  const artists = useFavoriteArtists()
  const query = { songs, albums, artists }[pref.tab]
  const sorted = useMemo(
    () => sortFavorites(query.data ?? [], pref.sort, pref.order),
    [query.data, pref.sort, pref.order],
  )

  const sortItems: MenuItem[] = [
    ...SORTS[pref.tab].map((s): MenuItem => ({
      id: s,
      label: t(`favorites.sort.${s}`),
      checked: s === pref.sort,
      onSelect: () => update({ ...pref, sort: s }),
    })),
    { type: 'separator' },
    ...(['Ascending', 'Descending'] as const).map((order): MenuItem => ({
      id: order,
      label: order === 'Ascending' ? '↑' : '↓',
      checked: pref.order === order,
      onSelect: () => update({ ...pref, order }),
    })),
  ]
  const count = (n: number | undefined) => (n ? ` ${n}` : '')

  return (
    <ShellPage
      title={t('favorites.title')}
      subtitle={query.data ? t('library.count', { count: query.data.length }) : undefined}
      gutter
      actions={
        <>
          <SegmentedControl<FavoriteTab>
            aria-label={t('favorites.title')}
            size="small"
            value={pref.tab}
            onChange={(tab) =>
              update({
                tab,
                sort: SORTS[tab].includes(pref.sort) ? pref.sort : 'name',
                order: pref.order,
              })
            }
            options={[
              { value: 'songs', label: `${t('nav.songs')}${count(songs.data?.length)}` },
              { value: 'albums', label: `${t('nav.albums')}${count(albums.data?.length)}` },
              { value: 'artists', label: `${t('nav.artists')}${count(artists.data?.length)}` },
            ]}
          />
          <MenuButton label={t('library.sort')} icon={<ArrowUpDown />} items={sortItems} />
        </>
      }
    >
      {query.isLoading ? (
        pref.tab === 'songs' ? (
          <div className="jrm-center">
            <Spinner size="large" />
          </div>
        ) : (
          <GridSkeleton kind={pref.tab === 'artists' ? 'round' : 'square'} />
        )
      ) : query.isError ? (
        <EmptyState
          title={t('library.error')}
          action={<Button onClick={() => void query.refetch()}>{t('library.retry')}</Button>}
        />
      ) : sorted.length === 0 ? (
        <EmptyState
          icon={<Heart />}
          title={t(`favorites.empty.${pref.tab}`)}
          message={t('favorites.emptyHint')}
        />
      ) : pref.tab === 'songs' ? (
        <>
          <div className="jrm-favorites__actions">
            <Button
              icon={<Play fill="currentColor" />}
              onClick={() => musicPlayer.play(sorted, 0, { shuffle: false })}
            >
              {t('favorites.playAll')}
            </Button>
            <Button
              variant="tinted"
              icon={<Shuffle />}
              onClick={() => musicPlayer.play(sorted, undefined, { shuffle: true })}
            >
              {t('actions.shuffle')}
            </Button>
          </div>
          <SongRows tracks={sorted} lead="artwork" showAlbum aria-label={t('favorites.title')} />
        </>
      ) : (
        <Grid kind={pref.tab === 'artists' ? 'round' : 'square'} aria-label={t('favorites.title')}>
          {sorted.map((item) => (
            <div key={item.Id}>
              {pref.tab === 'albums' ? <AlbumCard album={item} /> : <ArtistCard artist={item} />}
            </div>
          ))}
        </Grid>
      )}
    </ShellPage>
  )
}
