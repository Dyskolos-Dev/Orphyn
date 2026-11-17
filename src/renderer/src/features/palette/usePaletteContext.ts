import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { INTERFACE_STYLES, toast, useInterfaceStyle, useTheme } from '@renderer/design'
import {
  musicPlayer,
  useCurrentTrack,
  useMusicPlayer,
  useNowPlayingUi,
} from '@renderer/features/music'
import { openPlaylistPicker } from '@renderer/features/music/playlists/ui'
import { pipAvailable, togglePip, usePipState } from '@renderer/features/pip/events'
import { useIsFavorite, useSession, useToggleFavorite } from '@renderer/jellyfin'
import { useLibraryRefresh } from '@renderer/shell/useLibraryRefresh'
import type { PaletteContext } from './registry'

/** Builds what commands can read and do from the live app state (player, theme, session...). */
export function usePaletteContext(): PaletteContext {
  const { t } = useTranslation('palette')
  const navigate = useNavigate()
  const track = useCurrentTrack()
  const status = useMusicPlayer((s) => s.status)
  const shuffle = useMusicPlayer((s) => s.shuffle)
  const repeat = useMusicPlayer((s) => s.repeat)
  const trackId = track?.Id ?? undefined
  const favorite = useIsFavorite(trackId)
  const toggleFavorite = useToggleFavorite()
  const showNowPlaying = useNowPlayingUi((s) => s.show)
  const pip = usePipState()
  const theme = useTheme()
  const style = useInterfaceStyle()
  const accounts = useSession((s) => s.accounts)
  const active = useSession((s) => s.account)
  const switchServer = useSession((s) => s.switchServer)
  const signOut = useSession((s) => s.signOut)
  const { refresh } = useLibraryRefresh()

  return useMemo<PaletteContext>(() => {
    return {
      navigate,
      player: {
        hasTrack: track !== undefined,
        playing: status === 'playing' || status === 'loading',
        shuffle,
        repeat,
        toggle: () => musicPlayer.toggle(),
        next: () => musicPlayer.next(),
        previous: () => musicPlayer.previous(),
        toggleShuffle: () => musicPlayer.getState().toggleShuffle(),
        cycleRepeat: () => musicPlayer.getState().cycleRepeat(),
      },
      favorite: {
        available: trackId !== undefined,
        value: favorite,
        toggle() {
          if (!trackId) return
          toggleFavorite.mutate({ id: trackId, value: !favorite })
          toast({ title: t(favorite ? 'toast.favoriteRemoved' : 'toast.favoriteAdded') })
        },
      },
      openNowPlaying: () => showNowPlaying('player'),
      openPip: pipAvailable() ? () => void togglePip('music', pip) : undefined,
      addToPlaylist() {
        if (!trackId) return
        // The picker opens as a menu: anchor it near the top of the window, where the palette was.
        openPlaylistPicker([trackId], { x: window.innerWidth / 2, y: window.innerHeight / 3 })
      },
      theme: { resolved: theme.resolvedTheme, setMode: theme.setMode },
      interfaceStyle: {
        current: style.style,
        styles: INTERFACE_STYLES,
        set: (s) => style.setStyle(s as (typeof INTERFACE_STYLES)[number]),
      },
      accounts: accounts.map((a) => ({ id: a.id, name: a.name, active: a.id === active?.id })),
      switchServer: (id) => void switchServer(id),
      signedIn: active !== null,
      signOut: () => active && void signOut(active.id),
      refreshLibrary() {
        toast({ title: t('toast.refreshed') })
        refresh()
      },
      checkForUpdates: window.orphyn?.updater
        ? () => {
            toast({ title: t('toast.updatesChecked') })
            void window.orphyn.updater.check().catch(() => undefined)
          }
        : undefined,
    }
  }, [
    navigate,
    track,
    status,
    shuffle,
    repeat,
    trackId,
    favorite,
    toggleFavorite,
    showNowPlaying,
    pip,
    theme.resolvedTheme,
    theme.setMode,
    style,
    accounts,
    active,
    switchServer,
    signOut,
    refresh,
    t,
  ])
}
