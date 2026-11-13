import { ListPlus, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { MenuItem } from '@renderer/design'
import type { BaseItemDto } from '@renderer/jellyfin'
import { useContainerMenu, useTrackMenu } from '../components/hooks'
import { askDeletePlaylist, openPlaylistPicker } from './ui'

/** "Add to Playlist…": opens the picker where the user just clicked. */
export function useAddToPlaylistItem() {
  const { t } = useTranslation('music')
  return (ids: string[]): MenuItem => ({
    label: t('playlistMenu.add'),
    icon: <ListPlus />,
    onSelect: () => openPlaylistPicker(ids),
  })
}

/** Track menu plus "Add to Playlist…" and, inside a playlist, "Remove from this Playlist". */
export function useTrackMenuWithPlaylists(opts?: { onRemove?: (track: BaseItemDto) => void }) {
  const { t } = useTranslation('music')
  const base = useTrackMenu()
  const addItem = useAddToPlaylistItem()
  return (track: BaseItemDto): MenuItem[] => {
    const items = base(track)
    items.splice(2, 0, addItem([track.Id ?? '']))
    if (opts?.onRemove) {
      items.push(
        { type: 'separator' },
        {
          label: t('playlistMenu.removeFromThis'),
          icon: <Trash2 />,
          destructive: true,
          shortcut: '⌫',
          onSelect: () => opts.onRemove?.(track),
        },
      )
    }
    return items
  }
}

/** Album / playlist / artist menu plus "Add to Playlist…" (albums) and "Delete Playlist…" (playlists). */
export function useContainerMenuWithPlaylists() {
  const { t } = useTranslation('music')
  const base = useContainerMenu()
  const addItem = useAddToPlaylistItem()
  return (item: BaseItemDto): MenuItem[] => {
    const items = base(item)
    if (item.Type === 'MusicAlbum') items.splice(5, 0, addItem([item.Id ?? '']))
    if (item.Type === 'Playlist') {
      items.push(
        { type: 'separator' },
        {
          label: t('playlistMenu.delete'),
          icon: <Trash2 />,
          destructive: true,
          onSelect: () => askDeletePlaylist(item),
        },
      )
    }
    return items
  }
}
