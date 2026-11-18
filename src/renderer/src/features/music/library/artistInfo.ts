import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { artworkApi } from '@renderer/bridge/artwork'
import { usePlaybackPreferences } from '@renderer/features/settings/preferences'
import { artistKey } from './artistGroups'
import {
  acceptWikipediaSummary,
  wikipediaSummaryUrl,
  type Bio,
  type WikipediaSummary,
} from './artistBio'

/**
 * Online artist info (popular titles, related artists, biography). Same approach as the artist
 * images: native lookups behind the "online artist info" setting, answers kept in localStorage
 * (hits 30 days, misses 7), network failures never stored. Setting off = no request at all.
 */
const PREFIX = 'orphyn.artistInfo.v1:'
const HIT_TTL_MS = 30 * 24 * 3600_000
const MISS_TTL_MS = 7 * 24 * 3600_000

interface Entry<T> {
  v: T | null
  at: number
}

function read<T>(key: string, now = Date.now()): { v: T | null } | undefined {
  try {
    const raw = globalThis.localStorage?.getItem(PREFIX + key)
    if (!raw) return undefined
    const e = JSON.parse(raw) as Entry<T>
    const empty = e.v == null || (Array.isArray(e.v) && e.v.length === 0)
    if (typeof e.at !== 'number' || now - e.at > (empty ? MISS_TTL_MS : HIT_TTL_MS))
      return undefined
    return { v: e.v }
  } catch {
    return undefined
  }
}

function write<T>(key: string, v: T | null): void {
  try {
    globalThis.localStorage?.setItem(PREFIX + key, JSON.stringify({ v, at: Date.now() }))
  } catch {
    // Storage full: the query cache still holds it for this session.
  }
}

async function cached<T>(key: string, load: () => Promise<T | null>): Promise<T | null> {
  const hit = read<T>(key)
  if (hit) return hit.v
  const v = await load() // a rejection propagates: not stored, the query may retry
  write(key, v)
  return v
}

export const fetchTopTitles = (name: string) =>
  cached<string[]>(`top:${artistKey(name)}`, () => artworkApi.topTracks(name.trim()))

export const fetchRelatedNames = (name: string) =>
  cached<string[]>(`related:${artistKey(name)}`, () => artworkApi.relatedArtists(name.trim()))

/** Wikipedia (CORS allowed, so straight from the page), then TheAudioDB through the native side. */
export function fetchOnlineBio(name: string, lang: 'fr' | 'en'): Promise<Bio | null> {
  return cached<Bio>(`bio:${lang}:${artistKey(name)}`, async () => {
    try {
      const res = await fetch(wikipediaSummaryUrl(name, lang), {
        headers: { Accept: 'application/json' },
      })
      if (res.ok) {
        const text = acceptWikipediaSummary((await res.json()) as WikipediaSummary)
        if (text) return { text, source: 'wikipedia' }
      }
    } catch {
      // Offline or blocked: try the second source.
    }
    const text = await artworkApi.artistBio(name.trim(), lang)
    return text ? { text, source: 'audiodb' } : null
  })
}

/** True when online artist info may be requested: setting on and the native bridge there. */
export function useOnlineArtistInfo(): boolean {
  const { artistImages } = usePlaybackPreferences()
  return artistImages && artworkApi.available()
}

const QUERY = { staleTime: Infinity, gcTime: 3600_000, retry: false } as const

export function useTopTitles(name: string | undefined) {
  const on = useOnlineArtistInfo()
  const n = name?.trim() ?? ''
  return useQuery({
    queryKey: ['artistInfo', 'top', artistKey(n)],
    queryFn: () => fetchTopTitles(n),
    enabled: on && n !== '',
    ...QUERY,
  })
}

export function useRelatedNames(name: string | undefined) {
  const on = useOnlineArtistInfo()
  const n = name?.trim() ?? ''
  return useQuery({
    queryKey: ['artistInfo', 'related', artistKey(n)],
    queryFn: () => fetchRelatedNames(n),
    enabled: on && n !== '',
    ...QUERY,
  })
}

/** Online biography in the UI language; only asked for when `wanted` (no server overview). */
export function useOnlineBio(name: string | undefined, wanted: boolean) {
  const on = useOnlineArtistInfo()
  const { i18n } = useTranslation()
  const lang = i18n.language?.startsWith('fr') ? 'fr' : 'en'
  const n = name?.trim() ?? ''
  return useQuery({
    queryKey: ['artistInfo', 'bio', lang, artistKey(n)],
    queryFn: () => fetchOnlineBio(n, lang),
    enabled: on && wanted && n !== '',
    ...QUERY,
  })
}
