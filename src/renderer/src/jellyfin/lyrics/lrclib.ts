import type { Lyrics } from '../types'

/**
 * Online lyrics fallback: LRCLIB (https://lrclib.net), a free, open lyrics database.
 * Synced lyrics (LRC) are preferred over plain text. Results, misses included (for a day), are
 * cached in memory and in localStorage so a track is looked up once.
 */

export const LRCLIB_URL = 'https://lrclib.net/api'
export const LRCLIB_SOURCE = 'LRCLIB'
export const LRCLIB_CACHE_KEY = 'orphyn.lyrics.lrclib'
export const MISS_TTL_MS = 24 * 60 * 60 * 1000
const MAX_STORED = 150
/** LRCLIB asks for matches within a couple of seconds of the real duration. */
const DURATION_TOLERANCE_S = 3

export interface OnlineLyrics extends Lyrics {
  source: typeof LRCLIB_SOURCE
}

export interface TrackQuery {
  title: string
  artist: string
  album?: string
  /** Seconds. */
  duration?: number
}

const TIME = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g

/** Parses LRC text. Returns null when it holds no timestamped line. Several stamps share a line. */
export function parseLrc(text: string): Lyrics | null {
  const lines: { text: string; startMs: number }[] = []
  for (const raw of text.split(/\r?\n/)) {
    const stamps = [...raw.matchAll(TIME)]
    if (stamps.length === 0) continue
    const content = raw.replace(TIME, '').trim()
    for (const m of stamps) {
      const frac = m[3] ? Number((m[3] + '00').slice(0, 3)) : 0
      lines.push({ text: content, startMs: Number(m[1]) * 60_000 + Number(m[2]) * 1000 + frac })
    }
  }
  if (lines.length === 0) return null
  lines.sort((a, b) => a.startMs - b.startMs)
  return { synced: true, lines }
}

export function parsePlain(text: string): Lyrics | null {
  const lines = text.split(/\r?\n/).map((t) => ({ text: t.trimEnd(), startMs: null }))
  // Blank lines inside the text are stanza breaks; drop them at both ends.
  while (lines.length > 0 && lines[0]?.text === '') lines.shift()
  while (lines.length > 0 && lines[lines.length - 1]?.text === '') lines.pop()
  return lines.length > 0 ? { synced: false, lines } : null
}

interface LrclibRecord {
  duration?: number | null
  instrumental?: boolean
  plainLyrics?: string | null
  syncedLyrics?: string | null
}

/** Synced first, plain otherwise; null for instrumentals or empty records. */
export function lyricsFromRecord(r: LrclibRecord | null | undefined): OnlineLyrics | null {
  if (!r || r.instrumental) return null
  const parsed =
    (r.syncedLyrics ? parseLrc(r.syncedLyrics) : null) ??
    (r.plainLyrics ? parsePlain(r.plainLyrics) : null)
  return parsed ? { ...parsed, source: LRCLIB_SOURCE } : null
}

/** Best search hit: synced beats plain, then the closest duration within tolerance. */
export function pickBest(records: readonly LrclibRecord[], q: TrackQuery): OnlineLyrics | null {
  const want = q.duration ?? 0
  const gap = (r: LrclibRecord) => Math.abs((r.duration ?? want) - want)
  const usable = records
    .filter((r) => lyricsFromRecord(r) && (q.duration == null || gap(r) <= DURATION_TOLERANCE_S))
    .sort((a, b) => Number(!!b.syncedLyrics) - Number(!!a.syncedLyrics) || gap(a) - gap(b))
  return lyricsFromRecord(usable[0])
}

export function cacheKey(q: TrackQuery): string {
  const n = (s: string | undefined) => (s ?? '').trim().toLowerCase().replace(/\s+/g, ' ')
  return [n(q.artist), n(q.title), n(q.album), Math.round(q.duration ?? 0)].join('|')
}

interface Entry {
  /** Lyrics, or null for a miss. */
  v: OnlineLyrics | null
  /** Stored at (ms). */
  t: number
}

const memory = new Map<string, Entry>()
let loaded = false

function load(): void {
  if (loaded) return
  loaded = true
  try {
    const raw = JSON.parse(globalThis.localStorage?.getItem(LRCLIB_CACHE_KEY) ?? 'null') as Record<
      string,
      Entry
    > | null
    if (raw && typeof raw === 'object') {
      for (const [k, e] of Object.entries(raw)) if (e && typeof e.t === 'number') memory.set(k, e)
    }
  } catch {
    // Corrupt storage: start empty.
  }
}

function persist(): void {
  try {
    // Keep the most recent entries so storage stays small.
    const entries = [...memory.entries()].sort((a, b) => a[1].t - b[1].t).slice(-MAX_STORED)
    globalThis.localStorage?.setItem(LRCLIB_CACHE_KEY, JSON.stringify(Object.fromEntries(entries)))
  } catch {
    // Storage full or unavailable: the memory cache still works.
  }
}

/** Cached lookup: `undefined` = unknown (go online), `null` = known miss, else the lyrics. */
export function readCache(q: TrackQuery, now = Date.now()): OnlineLyrics | null | undefined {
  load()
  const e = memory.get(cacheKey(q))
  if (!e) return undefined
  if (e.v === null && now - e.t > MISS_TTL_MS) return undefined
  return e.v
}

function writeCache(q: TrackQuery, v: OnlineLyrics | null): void {
  load()
  memory.set(cacheKey(q), { v, t: Date.now() })
  persist()
}

/** Forgets everything (memory and storage). */
export function clearLrclibCache(): void {
  memory.clear()
  loaded = true
  try {
    globalThis.localStorage?.removeItem(LRCLIB_CACHE_KEY)
  } catch {
    // ignore
  }
}

/** Drops the in-memory copy so the next read goes back to storage (tests). */
export function resetLrclibMemory(): void {
  memory.clear()
  loaded = false
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T | null> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (res.status === 404) return null
  if (!res.ok) throw new Error(`LRCLIB ${res.status}`)
  return (await res.json()) as T
}

/**
 * Looks lyrics up for a track. Resolves to null when LRCLIB has none. Network failures throw
 * (and are not cached) so a flaky connection is retried later.
 */
export async function fetchLrclib(
  q: TrackQuery,
  signal?: AbortSignal,
): Promise<OnlineLyrics | null> {
  if (!q.title.trim() || !q.artist.trim()) return null
  const cached = readCache(q)
  if (cached !== undefined) return cached

  const exact = new URLSearchParams({ artist_name: q.artist, track_name: q.title })
  if (q.album) exact.set('album_name', q.album)
  if (q.duration && q.duration > 0) exact.set('duration', String(Math.round(q.duration)))
  let found = lyricsFromRecord(await getJson<LrclibRecord>(`${LRCLIB_URL}/get?${exact}`, signal))

  if (!found) {
    const search = new URLSearchParams({ artist_name: q.artist, track_name: q.title })
    const hits = await getJson<LrclibRecord[]>(`${LRCLIB_URL}/search?${search}`, signal)
    found = pickBest(Array.isArray(hits) ? hits : [], q)
  }
  writeCache(q, found)
  return found
}
