import { useCallback, useEffect, useState } from 'react'
import { artworkApi } from '@renderer/bridge/artwork'
import { imageUrl, type BaseItemDto } from '@renderer/jellyfin'
import { usePlaybackPreferences } from '@renderer/features/settings/preferences'
import { fetchArtistImage, readCachedArtistImage } from './artistImages'

export interface ArtistImage {
  /** Jellyfin image if the artist has one, else the online one, else `undefined`. */
  src: string | undefined
  /** Attach to the card's wrapper: the online lookup only starts once it is on screen. */
  ref: (el: Element | null) => void
}

interface Options {
  /** Jellyfin image width (CSS px of the request). */
  maxWidth?: number
  /** Look up immediately instead of waiting for the element to be visible. */
  eager?: boolean
}

/**
 * Artist picture. Order: the server's own image, then (setting on, native bridge there) the
 * online one, then nothing (the caller's fallback). Never writes to the server; with the
 * setting off no request is made.
 */
export function useArtistImage(
  artist: BaseItemDto | undefined,
  { maxWidth = 640, eager = false }: Options = {},
): ArtistImage {
  const { artistImages } = usePlaybackPreferences()
  const server = artist ? imageUrl(artist, 'Primary', { maxWidth, quality: 80 }) : undefined
  const name = artist?.Name?.trim() ?? ''
  const wanted = artistImages && !server && name !== '' && artworkApi.available()

  const [el, setEl] = useState<Element | null>(null)
  const [seen, setSeen] = useState(false)
  const ref = useCallback((node: Element | null) => setEl(node), [])
  useEffect(() => {
    if (!wanted || seen) return
    if (eager || typeof IntersectionObserver === 'undefined') {
      setSeen(true)
      return
    }
    if (!el) return
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) setSeen(true)
      },
      { rootMargin: '300px' },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [wanted, seen, eager, el])

  const [online, setOnline] = useState<{ name: string; url: string | null | undefined }>(() => ({
    name,
    url: wanted ? readCachedArtistImage(name) : undefined,
  }))
  useEffect(() => {
    if (!wanted || !seen) return
    let cancelled = false
    void fetchArtistImage(name).then((url) => {
      if (!cancelled) setOnline({ name, url })
    })
    return () => {
      cancelled = true
    }
  }, [wanted, seen, name])

  if (server) return { src: server, ref }
  if (!wanted) return { src: undefined, ref }
  const fromCache = readCachedArtistImage(name)
  const url = online.name === name && online.url !== undefined ? online.url : fromCache
  return { src: url ?? undefined, ref }
}
