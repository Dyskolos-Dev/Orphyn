import { Heart, ListMusic, Plus } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { Button, Menu, Sheet, TextField, type MenuItem } from '@renderer/design'
import { useUserPlaylists } from '@renderer/jellyfin'
import { registerShellExtension } from '@renderer/shell'
import { usePlaylistToasts } from './actions'
import { hasDragItems, playlistIdFromHref, readDragItems } from './dnd'
import { openNewPlaylist, trackPointer, usePlaylistUi } from './ui'

/** The "New Playlist" sheet, shared by the Playlists page, the picker and the multi-select bar. */
function NewPlaylistSheet() {
  const { t } = useTranslation('music')
  const sheet = usePlaylistUi((s) => s.sheet)
  const navigate = useNavigate()
  const { createWith } = usePlaylistToasts()
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [busy, setBusy] = useState(false)

  const close = () => {
    usePlaylistUi.setState({ sheet: null })
    setName('')
    setDescription('')
  }
  const submit = async () => {
    if (!name.trim() || !sheet || busy) return
    setBusy(true)
    const id = await createWith(name, sheet.itemIds, description)
    setBusy(false)
    if (id) {
      const hadItems = sheet.itemIds.length > 0
      close()
      if (!hadItems) void navigate(`/music/playlist/${id}`)
    }
  }

  return (
    <Sheet
      open={!!sheet}
      onClose={close}
      title={t('playlistSheet.title')}
      size="small"
      footer={
        <>
          <Button variant="gray" onClick={close}>
            {t('playlistSheet.cancel')}
          </Button>
          <Button disabled={!name.trim() || busy} onClick={() => void submit()}>
            {t('playlistSheet.create')}
          </Button>
        </>
      }
    >
      <form
        className="jrm-sheet-form"
        onSubmit={(e) => {
          e.preventDefault()
          void submit()
        }}
      >
        <TextField
          label={t('playlistSheet.name')}
          value={name}
          maxLength={120}
          autoFocus
          onChange={(e) => setName(e.target.value)}
        />
        <TextField
          label={t('playlistSheet.description')}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
      </form>
    </Sheet>
  )
}

/** Confirmation before deleting a playlist. */
function DeletePlaylistSheet() {
  const { t } = useTranslation('music')
  const deleting = usePlaylistUi((s) => s.deleting)
  const { deletePlaylist } = usePlaylistToasts()
  const close = () => usePlaylistUi.setState({ deleting: null })
  return (
    <Sheet
      open={!!deleting}
      onClose={close}
      size="small"
      title={t('playlistDelete.title', { name: deleting?.playlist.Name })}
      description={t('playlistDelete.message')}
      footer={
        <>
          <Button variant="gray" onClick={close}>
            {t('playlistSheet.cancel')}
          </Button>
          <Button
            destructive
            onClick={() => {
              const d = deleting
              close()
              if (d) void deletePlaylist(d.playlist).then((ok) => ok && d.onDeleted?.())
            }}
          >
            {t('playlistDelete.confirm')}
          </Button>
        </>
      }
    />
  )
}

/** Picker listing the user's playlists (opened by "Add to Playlist…" and the selection bar). */
function PlaylistPicker() {
  const { t } = useTranslation('music')
  const picker = usePlaylistUi((s) => s.picker)
  const playlists = useUserPlaylists()
  const { addTo } = usePlaylistToasts()
  if (!picker) return null
  const items: MenuItem[] = [
    {
      label: t('playlistMenu.new'),
      icon: <Plus />,
      onSelect: () => openNewPlaylist(picker.ids),
    },
    ...(playlists.data?.length ? [{ type: 'separator' } as const] : []),
    ...(playlists.data ?? []).map((p): MenuItem => ({
      id: p.Id ?? '',
      label: p.Name ?? '',
      icon: <ListMusic />,
      onSelect: () => void addTo(p, picker.ids),
    })),
  ]
  return (
    <Menu
      open
      anchor={picker.anchor}
      items={items}
      aria-label={t('playlistMenu.add')}
      onClose={() => usePlaylistUi.setState({ picker: null })}
    />
  )
}

/** Sidebar: Favourites + the user's playlists, plus dropping tracks / albums on a playlist link. */
function useSidebarEntries() {
  const { t } = useTranslation('music')
  const playlists = useUserPlaylists()
  const list = playlists.data
  useEffect(() => {
    return registerShellExtension({
      id: 'music-playlists',
      sidebarItems: [
        {
          id: 'favorites',
          section: 'music',
          label: t('nav.favorites'),
          icon: <Heart />,
          to: '/music/favorites',
          order: 35,
        },
        ...(list ?? []).map((p, i) => ({
          id: `playlist.${p.Id}`,
          section: 'music' as const,
          label: p.Name ?? '',
          icon: <ListMusic />,
          to: `/music/playlist/${p.Id}`,
          order: 100 + i,
        })),
      ],
    })
  }, [list, t])
}

function useSidebarDrops() {
  const { addTo } = usePlaylistToasts()
  const { data } = useUserPlaylists()
  useEffect(() => {
    let hovered: HTMLElement | null = null
    const clear = () => {
      hovered?.classList.remove('jrm-drop-target')
      hovered = null
    }
    const target = (e: DragEvent) => {
      if (!hasDragItems(e.dataTransfer)) return null
      const a = (e.target as HTMLElement | null)?.closest?.('a')
      const id = playlistIdFromHref(a?.getAttribute('href') ?? null)
      return a && id ? { a, id } : null
    }
    const over = (e: DragEvent) => {
      const t = target(e)
      if (!t) return clear()
      e.preventDefault()
      if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
      if (hovered !== t.a) {
        clear()
        hovered = t.a
        t.a.classList.add('jrm-drop-target')
      }
    }
    const drop = (e: DragEvent) => {
      const t = target(e)
      clear()
      if (!t) return
      e.preventDefault()
      const payload = readDragItems(e.dataTransfer)
      const playlist = data?.find((p) => p.Id === t.id)
      if (payload && playlist) void addTo(playlist, payload.ids)
    }
    document.addEventListener('dragover', over)
    document.addEventListener('dragleave', clear)
    document.addEventListener('drop', drop)
    document.addEventListener('dragend', clear)
    return () => {
      document.removeEventListener('dragover', over)
      document.removeEventListener('dragleave', clear)
      document.removeEventListener('drop', drop)
      document.removeEventListener('dragend', clear)
    }
  }, [addTo, data])
}

/** Always mounted by the shell (registered below): dialogs, sidebar entries and drop targets. */
export function PlaylistHost() {
  useEffect(() => trackPointer(), [])
  useSidebarEntries()
  useSidebarDrops()
  return (
    <>
      <NewPlaylistSheet />
      <DeletePlaylistSheet />
      <PlaylistPicker />
    </>
  )
}
