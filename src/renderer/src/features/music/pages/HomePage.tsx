import { Music } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState, Shelf, SquareCard } from '@renderer/design'
import {
  flattenPages,
  useAlbumArtists,
  useAlbums,
  useInstantMix,
  useItems,
  usePlaylists,
  type BaseItemDto,
} from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { AlbumCard, ArtistCard, PlaylistCard } from '../components/Cards'
import { art } from '../components/hooks'
import { musicPlayer } from '../player/api'
import { trackArtist } from '../player/track'

const SHELF = 20

/** "Listen Now": recently played, recently added, favourites, playlists and an instant mix. */
export function HomePage() {
  const { t } = useTranslation('music')
  const recentTracks = useItems({
    includeItemTypes: ['Audio'],
    filters: ['IsPlayed'],
    sortBy: ['DatePlayed'],
    sortOrder: 'Descending',
    pageSize: 40,
  })
  const added = useAlbums({ sortBy: ['DateCreated'], sortOrder: 'Descending', pageSize: SHELF })
  const favAlbums = useAlbums({ isFavorite: true, pageSize: SHELF })
  const favArtists = useAlbumArtists({ isFavorite: true, pageSize: SHELF })
  const playlists = usePlaylists({ pageSize: SHELF })

  // Recently played songs, collapsed to the albums they belong to.
  const recentAlbums = useMemo(() => {
    const seen = new Set<string>()
    const out: BaseItemDto[] = []
    for (const track of flattenPages(recentTracks.data)) {
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
    return out.slice(0, SHELF)
  }, [recentTracks.data])

  const addedAlbums = flattenPages(added.data)
  // Instant mix seeded by the last played song, else by the newest album.
  const seed = flattenPages(recentTracks.data)[0] ?? addedAlbums[0]
  const mix = useInstantMix(seed?.Id, SHELF)
  const mixTracks = mix.data ?? []

  const favAlbumItems = flattenPages(favAlbums.data)
  const favArtistItems = flattenPages(favArtists.data)
  const playlistItems = flattenPages(playlists.data)

  const loading = recentTracks.isLoading || added.isLoading
  const empty =
    !loading &&
    recentAlbums.length === 0 &&
    addedAlbums.length === 0 &&
    favAlbumItems.length === 0 &&
    favArtistItems.length === 0 &&
    playlistItems.length === 0

  return (
    <ShellPage title={t('home.title')}>
      {empty && (
        <EmptyState icon={<Music />} title={t('home.empty')} message={t('home.emptyHint')} />
      )}

      {recentAlbums.length > 0 && (
        <Shelf title={t('home.recentlyPlayed')} kind="square">
          {recentAlbums.map((a) => (
            <AlbumCard key={a.Id} album={a} />
          ))}
        </Shelf>
      )}
      {addedAlbums.length > 0 && (
        <Shelf title={t('home.recentlyAdded')} kind="square" seeAllTo="/music/albums">
          {addedAlbums.map((a) => (
            <AlbumCard key={a.Id} album={a} />
          ))}
        </Shelf>
      )}
      {mixTracks.length > 0 && seed && (
        <Shelf
          title={t('home.instantMix')}
          subtitle={t('home.instantMixFrom', { name: seed.Name })}
          kind="square"
        >
          {mixTracks.map((track, i) => (
            <SquareCard
              key={track.Id}
              title={track.Name ?? ''}
              subtitle={trackArtist(track)}
              image={art(track)}
              onClick={() => musicPlayer.play(mixTracks, i, { shuffle: false })}
              onPlay={() => musicPlayer.play(mixTracks, i, { shuffle: false })}
            />
          ))}
        </Shelf>
      )}
      {favAlbumItems.length > 0 && (
        <Shelf title={t('home.favoriteAlbums')} kind="square" seeAllTo="/music/albums">
          {favAlbumItems.map((a) => (
            <AlbumCard key={a.Id} album={a} />
          ))}
        </Shelf>
      )}
      {favArtistItems.length > 0 && (
        <Shelf title={t('home.favoriteArtists')} kind="round" seeAllTo="/music/artists">
          {favArtistItems.map((a) => (
            <ArtistCard key={a.Id} artist={a} />
          ))}
        </Shelf>
      )}
      {playlistItems.length > 0 && (
        <Shelf title={t('home.playlists')} kind="square" seeAllTo="/music/playlists">
          {playlistItems.map((p) => (
            <PlaylistCard key={p.Id} playlist={p} />
          ))}
        </Shelf>
      )}
    </ShellPage>
  )
}
