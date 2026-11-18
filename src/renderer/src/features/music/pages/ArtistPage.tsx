import { useMemo } from 'react'
import { User } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router'
import { EmptyState, Spinner } from '@renderer/design'
import { useArtistLists, useItem, useItems, useSimilar } from '@renderer/jellyfin'
import { usePlaybackPreferences } from '@renderer/features/settings/preferences'
import { ShellPage } from '@renderer/shell'
import { cleanOverview, type Bio } from '../library/artistBio'
import { groupArtists, groupIdsOf } from '../library/artistGroups'
import {
  useOnlineArtistInfo,
  useOnlineBio,
  useRelatedNames,
  useTopTitles,
} from '../library/artistInfo'
import { buildDiscography } from '../library/releaseType'
import { mergeSimilar } from '../library/similarArtists'
import { musicPlayer } from '../player/api'
import { ArtistAbout } from './artist/ArtistAbout'
import { ArtistHeader } from './artist/ArtistHeader'
import { Discography } from './artist/Discography'
import { PopularSongs } from './artist/PopularSongs'
import { SimilarArtists } from './artist/SimilarArtists'

const ALBUM_FIELDS = [
  'PrimaryImageAspectRatio',
  'Genres',
  'ProductionYear',
  'DateCreated',
  'ChildCount',
]
const EMPTY: never[] = []

export function ArtistPage() {
  const { t } = useTranslation('music')
  const { id } = useParams()
  const artist = useItem(id)
  const { groupArtists: grouping } = usePlaybackPreferences()
  const online = useOnlineArtistInfo()
  const lists = useArtistLists(grouping || online)
  // Every Jellyfin artist merged with this one (just this id when grouping is off).
  const ids = useMemo(() => {
    if (!id) return []
    if (!grouping || !lists.albumArtists) return [id]
    return groupIdsOf(groupArtists(lists.albumArtists, lists.everyone), id)
  }, [id, grouping, lists.albumArtists, lists.everyone])

  const albums = useItems({
    includeItemTypes: ['MusicAlbum'],
    artistIds: ids.length > 0 ? ids : undefined,
    sortBy: ['ProductionYear', 'SortName'],
    sortOrder: 'Descending',
    fields: ALBUM_FIELDS,
    pageSize: 200,
    enabled: ids.length > 0,
  })
  const songsQuery = useItems({
    includeItemTypes: ['Audio'],
    artistIds: ids,
    sortBy: ['Album', 'ParentIndexNumber', 'IndexNumber', 'SortName'],
    pageSize: 1000,
    enabled: ids.length > 0,
  })
  const similar = useSimilar(id, 12)

  const name = artist.data?.Name ?? undefined
  const overview = cleanOverview(artist.data?.Overview)
  const topTitles = useTopTitles(name)
  const related = useRelatedNames(name)
  const onlineBio = useOnlineBio(name, !!artist.data && overview === '')

  const songs = useMemo(
    () => songsQuery.data?.pages.flatMap((p) => p.items) ?? EMPTY,
    [songsQuery.data],
  )
  const discography = useMemo(
    () => buildDiscography(albums.data?.pages.flatMap((p) => p.items) ?? EMPTY, ids),
    [albums.data, ids],
  )
  const libraryArtists = useMemo(
    () => [...(lists.albumArtists ?? EMPTY), ...(lists.everyone ?? EMPTY)],
    [lists.albumArtists, lists.everyone],
  )
  const similarArtists = useMemo(
    () =>
      mergeSimilar(similar.data ?? EMPTY, related.data ?? EMPTY, libraryArtists, ids, name ?? ''),
    [similar.data, related.data, libraryArtists, ids, name],
  )

  const playAll = (shuffle: boolean) => {
    if (songs.length > 0) musicPlayer.play(songs, shuffle ? undefined : 0, { shuffle })
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

  const bio: Bio | undefined = overview
    ? { text: overview, source: 'jellyfin' }
    : (onlineBio.data ?? undefined)

  return (
    <ShellPage title={item.Name ?? ''} header="inline" gutter>
      <ArtistHeader artist={item} onPlay={() => playAll(false)} onShuffle={() => playAll(true)} />
      <PopularSongs
        songs={songs}
        ranking={topTitles.data ?? EMPTY}
        loading={songsQuery.isLoading || ids.length === 0 || (online && topTitles.isLoading)}
      />
      <Discography data={discography} />
      <ArtistAbout bio={bio} loading={overview === '' && onlineBio.isLoading} />
      <SimilarArtists
        artists={similarArtists}
        loading={similar.isLoading || (online && related.isLoading)}
      />
    </ShellPage>
  )
}
