import { Heart, Disc3, ListEnd, ListStart, Play, Shuffle, User } from 'lucide-react'
import { useEffect, useRef, useState, type RefObject } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import type { MenuItem } from '@renderer/design'
import { imageUrl, useToggleFavorite, type BaseItemDto } from '@renderer/jellyfin'
import { useShellContext } from '@renderer/shell/context'
import { musicPlayer } from '../player/api'
import { trackArtistId } from '../player/track'

export const isFavorite = (item: BaseItemDto): boolean => !!item.UserData?.IsFavorite

/** Square artwork URL of an item at roughly `size` CSS pixels (doubled for hi-dpi). */
export const art = (item: BaseItemDto | undefined, size = 320): string | undefined =>
  item
    ? imageUrl(item, 'Primary', { maxWidth: size * 2, maxHeight: size * 2, quality: 80 })
    : undefined

/** Context menu for an album, playlist or artist card. */
export function useContainerMenu() {
  const { t } = useTranslation('music')
  const fav = useToggleFavorite()
  return (item: BaseItemDto): MenuItem[] => {
    const id = item.Id ?? ''
    return [
      { label: t('actions.play'), icon: <Play />, onSelect: () => void musicPlayer.playItem(id) },
      {
        label: t('actions.shuffle'),
        icon: <Shuffle />,
        onSelect: () => void musicPlayer.playItem(id, { shuffle: true }),
      },
      { type: 'separator' },
      {
        label: t('actions.playNext'),
        icon: <ListStart />,
        onSelect: () => void musicPlayer.queueItem(id, 'next'),
      },
      {
        label: t('actions.addToQueue'),
        icon: <ListEnd />,
        onSelect: () => void musicPlayer.queueItem(id, 'last'),
      },
      { type: 'separator' },
      {
        label: isFavorite(item) ? t('actions.unfavorite') : t('actions.favorite'),
        icon: <Heart />,
        onSelect: () => fav.mutate({ id, value: !isFavorite(item) }),
      },
    ]
  }
}

/** Context menu for a single song. */
export function useTrackMenu() {
  const { t } = useTranslation('music')
  const navigate = useNavigate()
  const fav = useToggleFavorite()
  return (track: BaseItemDto): MenuItem[] => {
    const artistId = trackArtistId(track)
    const items: MenuItem[] = [
      {
        label: t('actions.playNext'),
        icon: <ListStart />,
        onSelect: () => musicPlayer.playNext([track]),
      },
      {
        label: t('actions.addToQueue'),
        icon: <ListEnd />,
        onSelect: () => musicPlayer.addToQueue([track]),
      },
      { type: 'separator' },
      {
        label: isFavorite(track) ? t('actions.unfavorite') : t('actions.favorite'),
        icon: <Heart />,
        onSelect: () => fav.mutate({ id: track.Id ?? '', value: !isFavorite(track) }),
      },
    ]
    if (track.AlbumId) {
      items.push({
        label: t('actions.goToAlbum'),
        icon: <Disc3 />,
        onSelect: () => void navigate(`/music/album/${track.AlbumId}`),
      })
    }
    if (artistId) {
      items.push({
        label: t('actions.goToArtist'),
        icon: <User />,
        onSelect: () => void navigate(`/music/artist/${artistId}`),
      })
    }
    return items
  }
}

/** Calls `onVisible` whenever the returned sentinel scrolls into view (infinite pagination). */
export function useEndReached(
  onVisible: () => void,
  enabled: boolean,
): RefObject<HTMLDivElement | null> {
  const ref = useRef<HTMLDivElement>(null)
  const cb = useRef(onVisible)
  cb.current = onVisible
  useEffect(() => {
    const el = ref.current
    if (!el || !enabled || typeof IntersectionObserver === 'undefined') return
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) cb.current()
      },
      { rootMargin: '600px 0px' },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [enabled])
  return ref
}

/**
 * Windowing for long fixed-height lists inside the shell's scroll area: returns the slice of rows
 * to render plus the padding that stands in for the others. The slice starts on an even index so
 * zebra striping stays stable while scrolling.
 */
export function useWindowedRows(count: number, rowHeight: number, overscan = 10) {
  const shell = useShellContext()
  const listRef = useRef<HTMLDivElement>(null)
  const [range, setRange] = useState({ start: 0, end: Math.min(count, 40) })

  useEffect(() => {
    const scroller = shell?.scrollRef.current
    const list = listRef.current
    if (!list) return
    const target: HTMLElement | Window = scroller ?? window
    const update = () => {
      const viewportH = scroller ? scroller.clientHeight : window.innerHeight
      const scrollTop = scroller ? scroller.scrollTop : window.scrollY
      const listTop = scroller
        ? list.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scrollTop
        : list.getBoundingClientRect().top + window.scrollY
      const first = Math.floor((scrollTop - listTop) / rowHeight) - overscan
      const last = Math.ceil((scrollTop - listTop + viewportH) / rowHeight) + overscan
      const start = Math.max(0, first - (first % 2))
      const end = Math.min(count, Math.max(last, start + 1))
      setRange((r) => (r.start === start && r.end === end ? r : { start, end }))
    }
    update()
    target.addEventListener('scroll', update, { passive: true })
    window.addEventListener('resize', update)
    return () => {
      target.removeEventListener('scroll', update)
      window.removeEventListener('resize', update)
    }
  }, [shell, count, rowHeight, overscan])

  const end = Math.min(range.end, count)
  const start = Math.min(range.start, end)
  return {
    listRef,
    start,
    end,
    paddingTop: start * rowHeight,
    paddingBottom: Math.max(0, count - end) * rowHeight,
  }
}
