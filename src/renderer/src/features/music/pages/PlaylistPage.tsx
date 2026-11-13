import { ListMusic, Pencil, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate, useParams } from 'react-router'
import { EmptyState, Spinner, Text, type MenuItem } from '@renderer/design'
import { useItem, usePlaylistItems } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { DetailHeader } from '../components/DetailHeader'
import { useFormatTotal } from '../components/format'
import { useContainerMenu } from '../components/hooks'
import { SongRows } from '../components/TrackRows'
import { musicPlayer } from '../player/api'
import { trackDuration } from '../player/track'
import { usePlaylistToasts } from '../playlists/actions'
import { InlineTitle } from '../playlists/InlineTitle'
import { PlaylistMosaic } from '../playlists/PlaylistMosaic'
import { askDeletePlaylist } from '../playlists/ui'

export function PlaylistPage() {
  const { t } = useTranslation('music')
  const { id } = useParams()
  const navigate = useNavigate()
  const playlist = useItem(id)
  const items = usePlaylistItems(id)
  const menu = useContainerMenu()
  const formatTotal = useFormatTotal()
  const { actions, fail } = usePlaylistToasts()
  const [renaming, setRenaming] = useState(false)

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

  const moreItems: MenuItem[] = [
    ...menu(item),
    { type: 'separator' },
    { label: t('playlist.rename'), icon: <Pencil />, onSelect: () => setRenaming(true) },
    {
      label: t('playlistMenu.delete'),
      icon: <Trash2 />,
      destructive: true,
      onSelect: () => askDeletePlaylist(item, () => void navigate('/music/playlists')),
    },
  ]

  return (
    <ShellPage title={item.Name ?? ''} header="inline" gutter>
      <DetailHeader
        item={item}
        title={
          <InlineTitle
            value={item.Name ?? ''}
            editing={renaming}
            onEditingChange={setRenaming}
            onCommit={(name) => void actions.rename(item.Id ?? '', name).catch(fail)}
          />
        }
        fallback={<PlaylistMosaic playlistId={item.Id ?? ''} size={232} />}
        subtitle={t('playlist.kind')}
        meta={
          songs.length
            ? `${t('album.footer', { count: songs.length })} · ${formatTotal(total)}`
            : undefined
        }
        description={item.Overview}
        onPlay={() => musicPlayer.play(songs, 0, { shuffle: false })}
        onShuffle={() => musicPlayer.play(songs, undefined, { shuffle: true })}
        menu={moreItems}
      />
      {songs.length === 0 ? (
        <EmptyState
          size="inline"
          icon={<ListMusic />}
          title={t('playlist.empty')}
          message={t('playlist.emptyHint')}
        />
      ) : (
        <>
          <SongRows
            tracks={songs}
            lead="artwork"
            showAlbum
            playlist={item}
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
