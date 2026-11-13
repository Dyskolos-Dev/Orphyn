import { Check, ListMusic, Trash2 } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from '@renderer/design'
import { usePlaylistActions, type BaseItemDto, type RemovedEntry } from '@renderer/jellyfin'

/** Playlist operations with their confirmation toasts (and Undo for removals). */
export function usePlaylistToasts() {
  const { t } = useTranslation('music')
  const actions = usePlaylistActions()
  return useMemo(() => {
    const fail = () => toast({ title: t('playlistToast.failed'), tone: 'error' })
    return {
      actions,
      async addTo(playlist: BaseItemDto, ids: string[]) {
        try {
          await actions.add(playlist.Id ?? '', ids)
          toast({
            title: t('playlistToast.added', { name: playlist.Name }),
            icon: <Check />,
          })
        } catch {
          fail()
        }
      },
      async createWith(name: string, ids: string[], description?: string): Promise<string | null> {
        try {
          const id = await actions.create(name, ids, { description })
          toast({
            title: ids.length
              ? t('playlistToast.added', { name: name.trim() })
              : t('playlistToast.created', { name: name.trim() }),
            icon: <ListMusic />,
          })
          return id
        } catch {
          fail()
          return null
        }
      },
      async removeEntries(playlist: BaseItemDto, entryIds: string[]) {
        try {
          const removed: RemovedEntry[] = await actions.removeEntries(playlist.Id ?? '', entryIds)
          toast({
            title: t('playlistToast.removed', { count: entryIds.length, name: playlist.Name }),
            icon: <Trash2 />,
            action: {
              label: t('playlistToast.undo'),
              onClick: () => void actions.restore(playlist.Id ?? '', removed).catch(fail),
            },
            duration: 6000,
          })
        } catch {
          fail()
        }
      },
      async deletePlaylist(playlist: BaseItemDto): Promise<boolean> {
        try {
          await actions.remove(playlist.Id ?? '')
          toast({ title: t('playlistToast.deleted', { name: playlist.Name }), icon: <Trash2 /> })
          return true
        } catch {
          fail()
          return false
        }
      },
      fail,
    }
  }, [actions, t])
}
