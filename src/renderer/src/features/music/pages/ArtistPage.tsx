import { useMemo } from 'react'
import { Heart, Play, Radio, Shuffle, User } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router'
import { Button, EmptyState, HeroBanner, IconButton, Shelf, Spinner, Text } from '@renderer/design'
import {
  imageUrl,
  useArtistLists,
  useItem,
  useItems,
  useSimilar,
  useToggleFavorite,
  type BaseItemDto,
} from '@renderer/jellyfin'
import { startRadio } from '@renderer/features/mixes'
import { usePlaybackPreferences } from '@renderer/features/settings/preferences'
import { ShellPage } from '@renderer/shell'
import { AlbumCard, ArtistCard } from '../components/Cards'
import { SongRows } from '../components/TrackRows'
import { groupArtists, groupIdsOf } from '../library/artistGroups'
import { useArtistImage } from '../library/useArtistImage'
import { musicPlayer } from '../player/api'

/** Albums with this many tracks or fewer are shown under "Singles & EPs". */
const MAX_SINGLE_TRACKS = 6
const ALBUM_FIELDS = [
  'PrimaryImageAspectRatio',
  'Genres',
  'ProductionYear',
  'DateCreated',
  'ChildCount',
]

function isSingle(album: BaseItemDto): boolean {
  if (/\b(single|ep)\b/i.test(album.Name ?? '')) return true
  return album.ChildCount != null && album.ChildCount > 0 && album.ChildCount <= MAX_SINGLE_TRACKS
}

export function ArtistPage() {
  const { t } = useTranslation('music')
  const { t: tr } = useTranslation('mixes')
  const { id } = useParams()
  const artist = useItem(id)
  const { groupArtists: grouping } = usePlaybackPreferences()
  const heroImage = useArtistImage(artist.data, { maxWidth: 1200, eager: true }).src
  const lists = useArtistLists(grouping)
  // Every Jellyfin artist merged with this one (just this id when grouping is off).
  const ids = useMemo(() => {
    if (!id) return []
    if (!grouping || !lists.albumArtists) return [id]
    return groupIdsOf(groupArtists(lists.albumArtists, lists.everyone), id)
  }, [id, grouping, lists.albumArtists, lists.everyone])
  const top = useItems({
    includeItemTypes: ['Audio'],
    artistIds: ids.length > 0 ? ids : undefined,
    sortBy: ['PlayCount', 'SortName'],
    sortOrder: 'Descending',
    pageSize: 10,
    enabled: !!id,
  })
  const albums = useItems({
    includeItemTypes: ['MusicAlbum'],
    artistIds: ids.length > 0 ? ids : undefined,
    sortBy: ['ProductionYear', 'SortName'],
    sortOrder: 'Descending',
    fields: ALBUM_FIELDS,
    pageSize: 200,
    enabled: !!id,
  })
  const allSongs = useItems({
    includeItemTypes: ['Audio'],
    artistIds: ids,
    sortBy: ['Album', 'ParentIndexNumber', 'IndexNumber', 'SortName'],
    pageSize: 1000,
    enabled: ids.length > 1,
  })
  const similar = useSimilar(id, 12)
  const fav = useToggleFavorite()

  const playAll = (shuffle: boolean) => {
    const songs = allSongs.data?.pages.flatMap((p) => p.items) ?? []
    if (ids.length > 1 && songs.length > 0)
      musicPlayer.play(songs, shuffle ? undefined : 0, { shuffle })
    else if (id) void musicPlayer.playArtist(id, shuffle ? { shuffle: true } : undefined)
  }

  if (artist.isLoading) {
    return (
      <ShellPage title="" header="inline">
        <div className="jrm-center">
          <Spinner size="large" />
        </div>
      </ShellPage>
    )
  }
  const item = artist.data
  if (!item || !id) {
    return (
      <ShellPage title={t('nav.artists')} header="inline">
        <EmptyState icon={<User />} title={t('artist.notFound')} />
      </ShellPage>
    )
  }

  const topSongs = top.data?.pages.flatMap((p) => p.items) ?? []
  const all = albums.data?.pages.flatMap((p) => p.items) ?? []
  const own = all.filter((a) => a.AlbumArtists?.some((x) => !!x.Id && ids.includes(x.Id)))
  const appearsOn = all.filter((a) => !a.AlbumArtists?.some((x) => !!x.Id && ids.includes(x.Id)))
  const singles = own.filter(isSingle)
  const fullAlbums = own.filter((a) => !isSingle(a))
  const similarArtists = (similar.data ?? []).filter((a) => a.Type === 'MusicArtist' || !a.Type)

  return (
    <ShellPage title={item.Name ?? ''} header="hero">
      <HeroBanner
        title={item.Name ?? ''}
        backdrop={
          imageUrl(item, 'Backdrop', { maxWidth: 1920, quality: 80 }) ?? heroImage
        }
        eyebrow={t('artist.eyebrow')}
        meta={item.Genres?.slice(0, 3).join(' · ')}
        size="medium"
        actions={
          <>
            <Button
              variant="contrast"
              size="large"
              icon={<Play fill="currentColor" />}
              onClick={() => playAll(false)}
            >
              {t('actions.play')}
            </Button>
            <Button
              variant="contrast"
              size="large"
              icon={<Shuffle />}
              onClick={() => playAll(true)}
            >
              {t('actions.shuffle')}
            </Button>
            <Button
              variant="contrast"
              size="large"
              icon={<Radio />}
              onClick={() => void startRadio(id)}
            >
              {tr('radio.start')}
            </Button>
            <IconButton
              icon={<Heart fill={item.UserData?.IsFavorite ? 'currentColor' : 'none'} />}
              label={item.UserData?.IsFavorite ? t('actions.unfavorite') : t('actions.favorite')}
              variant="gray"
              pressed={!!item.UserData?.IsFavorite}
              onClick={() => fav.mutate({ id, value: !item.UserData?.IsFavorite })}
            />
          </>
        }
      />

      {topSongs.length > 0 && (
        <section className="jrm-section">
          <Text variant="title-2" className="jrm-section__title">
            {t('artist.topSongs')}
          </Text>
          <SongRows tracks={topSongs} lead="artwork" showAlbum aria-label={t('artist.topSongs')} />
        </section>
      )}
      {fullAlbums.length > 0 && (
        <Shelf title={t('artist.albums')} kind="square">
          {fullAlbums.map((a) => (
            <AlbumCard key={a.Id} album={a} />
          ))}
        </Shelf>
      )}
      {singles.length > 0 && (
        <Shelf title={t('artist.singles')} kind="square">
          {singles.map((a) => (
            <AlbumCard key={a.Id} album={a} />
          ))}
        </Shelf>
      )}
      {appearsOn.length > 0 && (
        <Shelf title={t('artist.appearsOn')} kind="square">
          {appearsOn.map((a) => (
            <AlbumCard key={a.Id} album={a} />
          ))}
        </Shelf>
      )}
      {similarArtists.length > 0 && (
        <Shelf title={t('artist.similar')} kind="round">
          {similarArtists.map((a) => (
            <ArtistCard key={a.Id} artist={a} />
          ))}
        </Shelf>
      )}
    </ShellPage>
  )
}
