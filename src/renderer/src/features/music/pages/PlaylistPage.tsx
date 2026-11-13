import { ListMusic } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router'
import { EmptyState, Spinner, Text } from '@renderer/design'
import { useItem, usePlaylistItems } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { DetailHeader } from '../components/DetailHeader'
import { useFormatTotal } from '../components/format'
import { useContainerMenu } from '../components/hooks'
import { SongRows } from '../components/TrackRows'
import { musicPlayer } from '../player/api'
import { trackDuration } from '../player/track'

export function PlaylistPage() {
  const { t } = useTranslation('music')
  const { id } = useParams()
  const playlist = useItem(id)
  const items = usePlaylistItems(id)
  const menu = useContainerMenu()
  const formatTotal = useFormatTotal()

  if (playlist.isLoading || items.isLoading) {
    return (
      <ShellPage title="" header="inline">
        <div className="jrm-center">
          <Spinner size="large" />
        </div>
      </ShellPage>
    )
  }
  const item = playlist.data
  if (!item) {
    return (
      <ShellPage title={t('nav.playlists')} header="inline">
        <EmptyState icon={<ListMusic />} title={t('playlist.notFound')} />
      </ShellPage>
    )
  }
  const songs = items.data ?? []
  const total = songs.reduce((sum, s) => sum + trackDuration(s), 0)

  return (
    <ShellPage title={item.Name ?? ''} header="inline" gutter>
      <DetailHeader
        item={item}
        subtitle={t('playlist.kind')}
        meta={
          songs.length
            ? `${t('album.footer', { count: songs.length })} · ${formatTotal(total)}`
            : undefined
        }
        description={item.Overview}
        onPlay={() => musicPlayer.play(songs, 0, { shuffle: false })}
        onShuffle={() => musicPlayer.play(songs, undefined, { shuffle: true })}
        menu={menu(item)}
      />
      {songs.length === 0 ? (
        <EmptyState size="inline" icon={<ListMusic />} title={t('playlist.empty')} />
      ) : (
        <>
          <SongRows
            tracks={songs}
            lead="artwork"
            showAlbum
            aria-label={item.Name ?? t('nav.playlists')}
          />
          <Text variant="footnote" color="secondary" className="jrm-footer">
            {t('album.footer', { count: songs.length })} · {formatTotal(total)}
          </Text>
        </>
      )}
    </ShellPage>
  )
}
