import { ListMusic } from 'lucide-react'
import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from '@renderer/design'
import { requireApi, useSession, usePlaylistActions } from '@renderer/jellyfin'
import * as ep from '@renderer/jellyfin/endpoints'
import { usePlaylistToasts } from '../music/playlists/actions'

/** Name of the Jellyfin playlist a year's top songs are saved to. */
export const topPlaylistName = (title: string): string => title.trim()

/**
 * "Save as playlist" for a year's top songs: creates the Jellyfin playlist, or, when one with that
 * name already exists, replaces its songs with the current ranking. Resolves with the playlist id.
 */
export function useSaveTopPlaylist() {
  const { t } = useTranslation('mixes')
  const actions = usePlaylistActions()
  const { createWith, fail } = usePlaylistToasts()

  return useCallback(
    async (title: string, trackIds: readonly string[]): Promise<string | null> => {
      const name = topPlaylistName(title)
      const ids = trackIds.filter(Boolean)
      if (!name || ids.length === 0) return null
      try {
        const userId = useSession.getState().account?.userId ?? ''
        const api = requireApi()
        const existing = (await ep.fetchUserPlaylists(api, userId)).find(
          (p) => (p.Name ?? '').trim().toLowerCase() === name.toLowerCase(),
        )
        if (!existing?.Id) return await createWith(name, [...ids])
        const entries = await ep.fetchPlaylistItems(api, userId, existing.Id)
        const entryIds = entries.flatMap((e) => e.PlaylistItemId ?? [])
        if (entryIds.length > 0) await actions.removeEntries(existing.Id, entryIds)
        await actions.add(existing.Id, ids)
        toast({ title: t('topYear.updated', { name }), icon: <ListMusic /> })
        return existing.Id
      } catch {
        fail()
        return null
      }
    },
    [actions, createWith, fail, t],
  )
}
