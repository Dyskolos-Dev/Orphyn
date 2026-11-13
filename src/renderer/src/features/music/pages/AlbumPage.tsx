import { Disc3 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Link, useParams } from 'react-router'
import { EmptyState, Shelf, Spinner, Text } from '@renderer/design'
import { useAlbumTracks, useItem, useItems } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { AlbumCard } from '../components/Cards'
import { DetailHeader } from '../components/DetailHeader'
import { useFormatTotal } from '../components/format'
import { useContainerMenuWithPlaylists } from '../playlists/menus'
import { SongRows } from '../components/TrackRows'
import { musicPlayer } from '../player/api'
import { trackDuration } from '../player/track'

export function AlbumPage() {
  const { t } = useTranslation('music')
  const { id } = useParams()
  const album = useItem(id)
  const tracks = useAlbumTracks(id)
  const menu = useContainerMenuWithPlaylists()
  const formatTotal = useFormatTotal()
  const artistId = album.data?.AlbumArtists?.[0]?.Id ?? undefined
  const more = useItems({
    includeItemTypes: ['MusicAlbum'],
    albumArtistIds: artistId ? [artistId] : undefined,
    sortBy: ['ProductionYear', 'SortName'],
    sortOrder: 'Descending',
    pageSize: 20,
    enabled: !!artistId,
  })

  if (album.isLoading || tracks.isLoading) {
    return (
      <ShellPage title="" header="inline">
        <div className="jrm-center">
          <Spinner size="large" />
        </div>
      </ShellPage>
    )
  }
  const item = album.data
  if (!item) {
    return (
      <ShellPage title={t('nav.albums')} header="inline">
        <EmptyState icon={<Disc3 />} title={t('album.notFound')} />
      </ShellPage>
    )
  }

  const songs = tracks.data ?? []
  const total = songs.reduce((sum, s) => sum + trackDuration(s), 0)
  const otherAlbums = (more.data?.pages.flatMap((p) => p.items) ?? []).filter(
    (a) => a.Id !== item.Id,
  )

  return (
    <ShellPage title={item.Name ?? ''} header="inline" gutter>
      <DetailHeader
        item={item}
        subtitle={
          artistId && item.AlbumArtist ? (
            <Link to={`/music/artist/${artistId}`} className="jrm-link jrm-link--accent">
              {item.AlbumArtist}
            </Link>
          ) : (
            item.AlbumArtist
          )
        }
        meta={[item.Genres?.[0], item.ProductionYear].filter(Boolean).join(' · ')}
        onPlay={() => musicPlayer.play(songs, 0, { shuffle: false })}
        onShuffle={() => musicPlayer.play(songs, undefined, { shuffle: true })}
        menu={menu(item)}
      />
      <SongRows
        tracks={songs}
        lead="number"
        hideArtist={item.AlbumArtist ?? undefined}
        groupByDisc
        aria-label={item.Name ?? t('nav.albums')}
      />
      {songs.length > 0 && (
        <Text variant="footnote" color="secondary" className="jrm-footer">
          {t('album.footer', { count: songs.length })} · {formatTotal(total)}
        </Text>
      )}
      {otherAlbums.length > 0 && (
        <div className="jrm-bleed">
          <Shelf title={t('album.moreBy', { artist: item.AlbumArtist })} kind="square">
            {otherAlbums.map((a) => (
              <AlbumCard key={a.Id} album={a} />
            ))}
          </Shelf>
        </div>
      )}
    </ShellPage>
  )
}
