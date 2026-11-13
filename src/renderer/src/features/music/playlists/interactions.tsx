import clsx from 'clsx'
import { Heart, ListEnd, ListPlus, Trash2, X } from 'lucide-react'
import {
  useEffect,
  useRef,
  useState,
  type DragEvent,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { IconButton, Button, Text, toast } from '@renderer/design'
import { dropTargetIndex, useToggleFavorite, type BaseItemDto } from '@renderer/jellyfin'
import { musicPlayer } from '../player/api'
import { usePlaylistToasts } from './actions'
import { gapAtY, writeDragItems } from './dnd'
import {
  EMPTY_SELECTION,
  clickSelection,
  pickSelected,
  pruneSelection,
  selectAll,
  type SelectionState,
} from './selection'
import { openPlaylistPicker } from './ui'

export interface TrackListOptions {
  tracks: readonly BaseItemDto[]
  /** Index in `tracks` of the first rendered row (windowed lists). */
  firstIndex?: number
  /** Set on a playlist page: enables reordering by drag and removal with the Delete key. */
  playlist?: BaseItemDto
}

/**
 * Selection (Shift / Ctrl / Cmd click), drag (to the sidebar playlists, or to reorder inside a
 * playlist) and the Delete key for a list of `TrackRow`s rendered with `rowClass(i)` inside an
 * `<ol ref={listRef} {...listProps}>`. Rows are marked through the DOM because the design-system
 * row does not expose these hooks.
 */
export function useTrackListInteractions({ tracks, firstIndex = 0, playlist }: TrackListOptions) {
  const [sel, setSel] = useState<SelectionState>(EMPTY_SELECTION)
  const [gap, setGap] = useState<number | null>(null)
  const listRef = useRef<HTMLOListElement>(null)
  const dragFrom = useRef<number | null>(null)
  const { actions, removeEntries, fail } = usePlaylistToasts()

  useEffect(() => {
    setSel((s) => pruneSelection(s, tracks.length))
  }, [tracks.length])

  // Mark rows (index + draggable) after every render: cheap, and rows come and go with windowing.
  useEffect(() => {
    listRef.current?.querySelectorAll<HTMLElement>('li.jrm-row').forEach((li, k) => {
      li.dataset['row'] = String(firstIndex + k)
      li.draggable = true
    })
  })

  const rowOf = (target: EventTarget | null): number | null => {
    const li = (target as HTMLElement | null)?.closest?.<HTMLElement>('li.jrm-row')
    return li?.dataset['row'] !== undefined ? Number(li.dataset['row']) : null
  }

  const removeIndexes = (indexes: number[]) => {
    const entryIds = indexes.flatMap((i) => tracks[i]?.PlaylistItemId ?? [])
    if (playlist && entryIds.length) {
      setSel(EMPTY_SELECTION)
      void removeEntries(playlist, entryIds)
    }
  }

  const listProps = {
    onClickCapture(e: MouseEvent<HTMLOListElement>) {
      const idx = rowOf(e.target)
      if (idx === null) return
      const mods = { shift: e.shiftKey, toggle: e.ctrlKey || e.metaKey }
      if (mods.shift || mods.toggle) {
        e.preventDefault()
        e.stopPropagation()
        setSel((s) => clickSelection(s, idx, mods))
      } else if (sel.selected.size > 0 && !(e.target as HTMLElement).closest('button, a')) {
        setSel(EMPTY_SELECTION)
      }
    },
    onKeyDown(e: KeyboardEvent<HTMLOListElement>) {
      const idx = rowOf(e.target)
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a' && idx !== null) {
        e.preventDefault()
        setSel(selectAll(tracks.length))
      } else if (e.key === 'Escape' && sel.selected.size) {
        setSel(EMPTY_SELECTION)
      } else if ((e.key === 'Delete' || e.key === 'Backspace') && playlist && idx !== null) {
        e.preventDefault()
        removeIndexes(sel.selected.has(idx) ? [...sel.selected].sort((a, b) => a - b) : [idx])
      }
    },
    onDragStart(e: DragEvent<HTMLOListElement>) {
      const idx = rowOf(e.target)
      if (idx === null) return
      const picked = sel.selected.has(idx)
        ? pickSelected(tracks, sel)
        : [tracks[idx] as BaseItemDto]
      dragFrom.current = playlist ? idx : null
      writeDragItems(e.dataTransfer, {
        ids: picked.map((t) => t.Id ?? '').filter(Boolean),
        label: picked.length === 1 ? (picked[0]?.Name ?? '') : `${picked.length}`,
      })
    },
    onDragOver(e: DragEvent<HTMLOListElement>) {
      if (dragFrom.current === null) return
      e.preventDefault()
      e.dataTransfer.dropEffect = 'move'
      const rows = Array.from(e.currentTarget.querySelectorAll('li.jrm-row')).map((li) =>
        li.getBoundingClientRect(),
      )
      const next = firstIndex + gapAtY(rows, e.clientY)
      setGap((g) => (g === next ? g : next))
    },
    onDrop(e: DragEvent<HTMLOListElement>) {
      const from = dragFrom.current
      const target = gap
      dragFrom.current = null
      setGap(null)
      if (from === null || target === null || !playlist) return
      e.preventDefault()
      const to = dropTargetIndex(from, target)
      const entry = tracks[from]?.PlaylistItemId
      if (to !== from && entry) {
        void actions.move(playlist.Id ?? '', entry, to).catch(fail)
      }
    },
    onDragEnd() {
      dragFrom.current = null
      setGap(null)
    },
  }

  const rowClass = (i: number) =>
    clsx(
      'jrm-row',
      gap === i && 'jrm-drop-before',
      gap === tracks.length && i === tracks.length - 1 && 'jrm-drop-after',
    )

  const selected = pickSelected(tracks, sel)
  const bar = (
    <SelectionBar
      tracks={selected}
      onClear={() => setSel(EMPTY_SELECTION)}
      onRemove={playlist ? () => removeIndexes([...sel.selected]) : undefined}
    />
  )

  return {
    listRef,
    listProps,
    rowClass,
    isSelected: (i: number) => sel.selected.has(i),
    selectedCount: sel.selected.size,
    bar,
    /** Removes one track (context menu); only meaningful with `playlist`. */
    removeTrack: (index: number) => removeIndexes([index]),
  }
}

function SelectionBar({
  tracks,
  onClear,
  onRemove,
}: {
  tracks: BaseItemDto[]
  onClear(): void
  onRemove?: () => void
}): ReactNode {
  const { t } = useTranslation('music')
  const fav = useToggleFavorite()
  if (tracks.length === 0 || typeof document === 'undefined') return null
  const allFavorite = tracks.every((x) => x.UserData?.IsFavorite)
  const ids = tracks.map((x) => x.Id ?? '').filter(Boolean)
  return createPortal(
    <div className="jrm-selbar jr-material-thick" role="toolbar" aria-label={t('selection.bar')}>
      <Text variant="callout" className="jrm-selbar__count">
        {t('selection.count', { count: tracks.length })}
      </Text>
      <Button
        variant="tinted"
        icon={<ListPlus />}
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect()
          openPlaylistPicker(ids, { x: r.left, y: r.top })
        }}
      >
        {t('playlistMenu.add')}
      </Button>
      <Button
        variant="tinted"
        icon={<ListEnd />}
        onClick={() => {
          musicPlayer.addToQueue(tracks)
          toast({ title: t('queue.addedLast') })
        }}
      >
        {t('actions.addToQueue')}
      </Button>
      <Button
        variant="tinted"
        icon={<Heart fill={allFavorite ? 'currentColor' : 'none'} />}
        onClick={() => ids.forEach((id) => fav.mutate({ id, value: !allFavorite }))}
      >
        {allFavorite ? t('actions.unfavorite') : t('actions.favorite')}
      </Button>
      {onRemove && (
        <Button variant="tinted" destructive icon={<Trash2 />} onClick={onRemove}>
          {t('playlistMenu.remove')}
        </Button>
      )}
      <IconButton icon={<X />} label={t('selection.clear')} variant="gray" onClick={onClear} />
    </div>,
    document.body,
  )
}
