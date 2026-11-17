import { useMemo, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Shelf, SquareCard, type ShelfItemKind } from '@renderer/design'
import { RowFrame, countOf, rowTitle, type HomeRow } from '@renderer/features/home'
import {
  flattenPages,
  useAlbumArtists,
  useAlbums,
  useArtistAlbums,
  useInstantMix,
  useItems,
  usePlaylists,
  useSongs,
  type BaseItemDto,
} from '@renderer/jellyfin'
import { MixRow } from '@renderer/features/mixes'
import { AlbumCard, ArtistCard, PlaylistCard } from '../components/Cards'
import { art } from '../components/hooks'
import { musicPlayer } from '../player/api'
import { trackArtist } from '../player/track'

function useTitle(row: HomeRow): string {
  const { t } = useTranslation('home')
  return rowTitle(t, 'music', row)
}

function MusicShelf({
  row,
  title,
  subtitle,
  kind = 'square',
  seeAllTo,
  count,
  children,
}: {
  row: HomeRow
  title: string
  subtitle?: string
  kind?: ShelfItemKind
  seeAllTo?: string
  /** How many cards `children` holds: nothing is drawn for 0. */
  count: number
  children: ReactNode
}) {
  if (count === 0) return null
  return (
    <RowFrame size={row.size}>
      <Shelf title={title} subtitle={subtitle} kind={kind} seeAllTo={seeAllTo}>
        {children}
      </Shelf>
    </RowFrame>
  )
}

const albumCards = (albums: BaseItemDto[]) => albums.map((a) => <AlbumCard key={a.Id} album={a} />)

function trackCards(tracks: BaseItemDto[]) {
  const play = (i: number) => musicPlayer.play(tracks, i, { shuffle: false })
  return tracks.map((track, i) => (
    <SquareCard
      key={track.Id}
      title={track.Name ?? ''}
      subtitle={trackArtist(track)}
      image={art(track)}
      onClick={() => play(i)}
      onPlay={() => play(i)}
    />
  ))
}

type RowProps = { row: HomeRow }

/** Last played songs: used by "Recently Played" and as the seed of the "Instant Mix". */
function useRecentTracks(pageSize: number) {
  return useItems({
    includeItemTypes: ['Audio'],
    filters: ['IsPlayed'],
    sortBy: ['DatePlayed'],
    sortOrder: 'Descending',
    pageSize,
  })
}

function RecentlyPlayedRow({ row }: RowProps) {
  const count = countOf(row, 'music')
  const recent = useRecentTracks(Math.max(40, count * 2))
  const title = useTitle(row)
  // Recently played songs, collapsed to the albums they belong to.
  const albums = useMemo(() => {
    const seen = new Set<string>()
    const out: BaseItemDto[] = []
    for (const track of flattenPages(recent.data)) {
      if (!track.AlbumId || seen.has(track.AlbumId)) continue
      seen.add(track.AlbumId)
      out.push({
        Id: track.AlbumId,
        Type: 'MusicAlbum',
        Name: track.Album,
        AlbumArtist: track.AlbumArtist,
        ProductionYear: track.ProductionYear,
        ImageTags: track.AlbumPrimaryImageTag ? { Primary: track.AlbumPrimaryImageTag } : undefined,
      })
    }
    return out.slice(0, count)
  }, [recent.data, count])
  return (
    <MusicShelf row={row} title={title} count={albums.length}>
      {albumCards(albums)}
    </MusicShelf>
  )
}

function RecentlyAddedRow({ row }: RowProps) {
  const q = useAlbums({
    sortBy: ['DateCreated'],
    sortOrder: 'Descending',
    pageSize: countOf(row, 'music'),
  })
  const albums = flattenPages(q.data)
  return (
    <MusicShelf row={row} title={useTitle(row)} count={albums.length} seeAllTo="/music/albums">
      {albumCards(albums)}
    </MusicShelf>
  )
}

function FavAlbumsRow({ row }: RowProps) {
  const q = useAlbums({ isFavorite: true, pageSize: countOf(row, 'music') })
  const albums = flattenPages(q.data)
  return (
    <MusicShelf row={row} title={useTitle(row)} count={albums.length} seeAllTo="/music/albums">
      {albumCards(albums)}
    </MusicShelf>
  )
}

function FavArtistsRow({ row }: RowProps) {
  const q = useAlbumArtists({ isFavorite: true, pageSize: countOf(row, 'music') })
  const artists = flattenPages(q.data)
  return (
    <MusicShelf
      row={row}
      title={useTitle(row)}
      kind="round"
      count={artists.length}
      seeAllTo="/music/artists"
    >
      {artists.map((a) => (
        <ArtistCard key={a.Id} artist={a} />
      ))}
    </MusicShelf>
  )
}

function FavSongsRow({ row }: RowProps) {
  const q = useSongs({ isFavorite: true, pageSize: countOf(row, 'music') })
  const tracks = flattenPages(q.data)
  return (
    <MusicShelf row={row} title={useTitle(row)} count={tracks.length} seeAllTo="/music/favorites">
      {trackCards(tracks)}
    </MusicShelf>
  )
}

function PlaylistsRow({ row }: RowProps) {
  const q = usePlaylists({ pageSize: countOf(row, 'music') })
  const playlists = flattenPages(q.data)
  return (
    <MusicShelf
      row={row}
      title={useTitle(row)}
      count={playlists.length}
      seeAllTo="/music/playlists"
    >
      {playlists.map((p) => (
        <PlaylistCard key={p.Id} playlist={p} />
      ))}
    </MusicShelf>
  )
}

/** Instant mix seeded by the last played song, else by the newest album. */
function InstantMixRow({ row }: RowProps) {
  const { t } = useTranslation('home')
  const last = useRecentTracks(1)
  const newest = useAlbums({ sortBy: ['DateCreated'], sortOrder: 'Descending', pageSize: 1 })
  const seed = flattenPages(last.data)[0] ?? flattenPages(newest.data)[0]
  const mix = useInstantMix(seed?.Id, countOf(row, 'music'))
  const tracks = mix.data ?? []
  return (
    <MusicShelf
      row={row}
      title={useTitle(row)}
      subtitle={seed ? t('instantMixFrom', { name: seed.Name ?? '' }) : undefined}
      count={seed ? tracks.length : 0}
    >
      {trackCards(tracks)}
    </MusicShelf>
  )
}

function GenreRow({ row }: RowProps) {
  const q = useAlbums({
    genres: row.param ? [row.param] : undefined,
    sortBy: ['DateCreated'],
    sortOrder: 'Descending',
    pageSize: countOf(row, 'music'),
    enabled: !!row.param,
  })
  const albums = flattenPages(q.data)
  return (
    <MusicShelf row={row} title={useTitle(row)} count={albums.length}>
      {albumCards(albums)}
    </MusicShelf>
  )
}

function ArtistRow({ row }: RowProps) {
  const q = useArtistAlbums(row.param)
  const albums = (q.data ?? []).slice(0, countOf(row, 'music'))
  return (
    <MusicShelf
      row={row}
      title={useTitle(row)}
      count={albums.length}
      seeAllTo={row.param ? `/music/artist/${row.param}` : undefined}
    >
      {albumCards(albums)}
    </MusicShelf>
  )
}

function RandomAlbumsRow({ row }: RowProps) {
  const q = useAlbums({ sortBy: ['Random'], pageSize: countOf(row, 'music') })
  const albums = flattenPages(q.data)
  return (
    <MusicShelf row={row} title={useTitle(row)} count={albums.length}>
      {albumCards(albums)}
    </MusicShelf>
  )
}

/** A shelf of the music home, by row type. */
export function MusicRow(props: RowProps) {
  switch (props.row.type) {
    case 'recentlyPlayed':
      return <RecentlyPlayedRow {...props} />
    case 'recentlyAdded':
      return <RecentlyAddedRow {...props} />
    case 'favAlbums':
      return <FavAlbumsRow {...props} />
    case 'favArtists':
      return <FavArtistsRow {...props} />
    case 'favSongs':
      return <FavSongsRow {...props} />
    case 'playlists':
      return <PlaylistsRow {...props} />
    case 'instantMix':
      return <InstantMixRow {...props} />
    case 'genre':
      return <GenreRow {...props} />
    case 'artist':
      return <ArtistRow {...props} />
    case 'randomAlbums':
      return <RandomAlbumsRow {...props} />
    case 'mixForYou':
    case 'mixDaily':
    case 'mixGenres':
      return <MixRow {...props} />
    default:
      return null
  }
}
