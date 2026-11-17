import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  LRCLIB_CACHE_KEY,
  MISS_TTL_MS,
  clearLrclibCache,
  fetchLrclib,
  lyricsFromRecord,
  parseLrc,
  pickBest,
  readCache,
  resetLrclibMemory,
} from './lrclib'

const q = { title: 'Song', artist: 'Band', album: 'Album', duration: 200.4 }
const json = (body: unknown, status = 200) =>
  Promise.resolve(new Response(JSON.stringify(body), { status }))

beforeEach(() => clearLrclibCache())
afterEach(() => vi.unstubAllGlobals())

describe('parseLrc', () => {
  it('reads timestamps, fractions, shared stamps and ignores tags', () => {
    const lrc = parseLrc('[ti:Song]\n[00:12.50] second\n[00:01.5][00:30.00]chorus\n[01:02:5] end')
    expect(lrc?.synced).toBe(true)
    expect(lrc?.lines).toEqual([
      { text: 'chorus', startMs: 1500 },
      { text: 'second', startMs: 12500 },
      { text: 'chorus', startMs: 30000 },
      { text: 'end', startMs: 62500 },
    ])
  })
  it('is null without timestamps', () => {
    expect(parseLrc('just words')).toBeNull()
  })
})

describe('lyricsFromRecord / pickBest', () => {
  it('prefers synced over plain and skips instrumentals', () => {
    const r = lyricsFromRecord({ plainLyrics: 'a\nb', syncedLyrics: '[00:01.00] a' })
    expect(r?.synced).toBe(true)
    expect(r?.source).toBe('LRCLIB')
    expect(lyricsFromRecord({ plainLyrics: 'a\nb' })?.synced).toBe(false)
    expect(lyricsFromRecord({ instrumental: true, plainLyrics: 'x' })).toBeNull()
  })
  it('picks a synced hit within the duration tolerance', () => {
    const hit = pickBest(
      [
        { duration: 400, syncedLyrics: '[00:01.00] far' },
        { duration: 200, plainLyrics: 'plain' },
        { duration: 201, syncedLyrics: '[00:01.00] near' },
      ],
      q,
    )
    expect(hit?.lines[0]?.text).toBe('near')
  })
})

describe('fetchLrclib', () => {
  it('uses /get and caches the result in memory and localStorage', async () => {
    const fetchMock = vi.fn<(url: string) => Promise<Response>>(() =>
      json({ syncedLyrics: '[00:02.00] hello', duration: 200 }),
    )
    vi.stubGlobal('fetch', fetchMock)
    const first = await fetchLrclib(q)
    expect(first?.lines).toEqual([{ text: 'hello', startMs: 2000 }])
    const url = String(fetchMock.mock.calls[0]?.[0])
    expect(url).toContain('/get?')
    expect(url).toContain('artist_name=Band')
    expect(url).toContain('duration=200')
    await fetchLrclib({ ...q, artist: ' band ' })
    expect(fetchMock).toHaveBeenCalledTimes(1)
    resetLrclibMemory() // new session: comes back from localStorage
    await fetchLrclib(q)
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(localStorage.getItem(LRCLIB_CACHE_KEY)).toContain('hello')
  })

  it('falls back to /search on a 404 and remembers misses for a day', async () => {
    const fetchMock = vi.fn((url: string) => (url.includes('/get?') ? json({}, 404) : json([])))
    vi.stubGlobal('fetch', fetchMock)
    expect(await fetchLrclib(q)).toBeNull()
    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(String(fetchMock.mock.calls[1]?.[0])).toContain('/search?')
    expect(await fetchLrclib(q)).toBeNull()
    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(readCache(q, Date.now() + MISS_TTL_MS + 1000)).toBeUndefined()
  })

  it('does not cache network failures', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => json({}, 500)),
    )
    await expect(fetchLrclib(q)).rejects.toThrow()
    expect(readCache(q)).toBeUndefined()
  })
})
