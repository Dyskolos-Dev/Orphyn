import { artworkApi } from '@renderer/bridge/artwork'
import { artistKey } from './artistGroups'

/**
 * Online artist images: renderer-side cache and request queue in front of the native lookup.
 * Hits are remembered 30 days, misses 7 days (localStorage, per normalised name); network
 * failures are not stored, only paused for a minute. At most `MAX_CONCURRENT` lookups run.
 */
export const HIT_TTL_MS = 30 * 24 * 3600_000
export const MISS_TTL_MS = 7 * 24 * 3600_000
export const ARTIST_IMAGE_KEY_PREFIX = 'orphyn.artistImage.v1:'
const MAX_CONCURRENT = 4
const FAILURE_PAUSE_MS = 60_000

interface Entry {
  url: string | null
  at: number
}

const pending = new Map<string, Promise<string | null | undefined>>()
const failedAt = new Map<string, number>()
const queue: Array<() => void> = []
let running = 0

function storageKey(name: string): string {
  return ARTIST_IMAGE_KEY_PREFIX + artistKey(name)
}

/** The cached answer: a URL, `null` (known miss) or `undefined` (never looked up / expired). */
export function readCachedArtistImage(name: string, now = Date.now()): string | null | undefined {
  try {
    const raw = globalThis.localStorage?.getItem(storageKey(name))
    if (!raw) return undefined
    const e = JSON.parse(raw) as Entry
    const ttl = e.url ? HIT_TTL_MS : MISS_TTL_MS
    if (typeof e.at !== 'number' || now - e.at > ttl) return undefined
    return typeof e.url === 'string' ? e.url : null
  } catch {
    return undefined
  }
}

function writeCache(name: string, url: string | null): void {
  try {
    globalThis.localStorage?.setItem(storageKey(name), JSON.stringify({ url, at: Date.now() }))
  } catch {
    // Storage full or unavailable: the answer still lives in memory for this session.
  }
}

function schedule<T>(task: () => Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const run = () => {
      running++
      task()
        .then(resolve, reject)
        .finally(() => {
          running--
          queue.shift()?.()
        })
    }
    if (running < MAX_CONCURRENT) run()
    else queue.push(run)
  })
}

/**
 * Resolves the online image of an artist. `undefined` = could not be determined right now
 * (offline, API error): the caller shows its placeholder and may ask again later.
 */
export function fetchArtistImage(name: string): Promise<string | null | undefined> {
  const key = artistKey(name)
  if (!key) return Promise.resolve(null)
  const cached = readCachedArtistImage(name)
  if (cached !== undefined) return Promise.resolve(cached)
  const failed = failedAt.get(key)
  if (failed && Date.now() - failed < FAILURE_PAUSE_MS) return Promise.resolve(undefined)
  const inflight = pending.get(key)
  if (inflight) return inflight
  const p = schedule(() => artworkApi.artistImage(name.trim()))
    .then((url) => {
      writeCache(name, url)
      return url
    })
    .catch(() => {
      failedAt.set(key, Date.now())
      return undefined
    })
    .finally(() => pending.delete(key))
  pending.set(key, p)
  return p
}

/** Tests only. */
export function resetArtistImageState(): void {
  pending.clear()
  failedAt.clear()
  queue.length = 0
  running = 0
}
