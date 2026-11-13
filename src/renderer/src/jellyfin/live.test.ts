// @vitest-environment node
/**
 * Opt-in live check against a real server (not part of the default run):
 *   ORPHYN_LIVE_URL=https://demo.jellyfin.org/stable ORPHYN_LIVE_USER=demo npx vitest run src/renderer/src/jellyfin/live.test.ts
 * Optional ORPHYN_LIVE_PASSWORD. Probes, logs in, lists libraries, resolves a playable movie URL and a
 * track URL, and fetches the first bytes of each to prove they play.
 */
import axios from 'axios'
import { describe, expect, it } from 'vitest'
import { login } from './auth'
import { createJellyfinApi } from './client'
import {
  fetchItemsPage,
  fetchLatest,
  fetchListPage,
  fetchLyrics,
  fetchNextUp,
  fetchResume,
  fetchSearch,
  fetchUserViews,
} from './endpoints'
import { getPlaybackInfo } from './playback/info'
import { probeServer } from './probe'

// tsconfig.web has no node types: read the environment through globalThis.
const env: Record<string, string | undefined> =
  (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env ?? {}
const url = env['ORPHYN_LIVE_URL']
const user = env['ORPHYN_LIVE_USER'] ?? 'demo'

async function firstBytes(target: string): Promise<number> {
  const res = await axios.get(target, {
    headers: { Range: 'bytes=0-1023' },
    responseType: 'arraybuffer',
    timeout: 30_000,
    validateStatus: () => true,
  })
  return res.status
}

describe.runIf(url)('live Jellyfin server', () => {
  it('probe, login, libraries, movie and track playback URLs', { timeout: 120_000 }, async () => {
    const probe = await probeServer(url as string)
    expect(probe.ok).toBe(true)
    if (!probe.ok) return
    console.log('server', probe.info.serverName, probe.info.version, probe.info.url)

    const auth = await login(
      probe.info.url,
      user,
      env['ORPHYN_LIVE_PASSWORD'] ?? '',
      'live-test-device',
    )
    expect(auth.ok).toBe(true)
    if (!auth.ok) return
    const api = createJellyfinApi({
      baseUrl: probe.info.url,
      deviceId: 'live-test-device',
      accessToken: auth.session.accessToken,
    })
    const userId = auth.session.userId

    const views = await fetchUserViews(api, userId)
    console.log('libraries', views.map((v) => `${v.Name}(${v.CollectionType})`).join(', '))
    expect(views.length).toBeGreaterThan(0)

    // Every other endpoint must at least answer with the expected shape.
    expect(Array.isArray(await fetchResume(api, userId, 'video'))).toBe(true)
    expect(Array.isArray(await fetchResume(api, userId, 'audio'))).toBe(true)
    expect(Array.isArray(await fetchNextUp(api, userId))).toBe(true)
    expect(Array.isArray(await fetchLatest(api, userId, views[0]!.Id!))).toBe(true)
    const albums = await fetchListPage(api, userId, 'albums', { pageSize: 2 }, 0)
    expect(albums.total).toBeGreaterThanOrEqual(albums.items.length)
    const artists = await fetchListPage(api, userId, 'albumArtists', { pageSize: 2 }, 0)
    expect(Array.isArray(artists.items)).toBe(true)
    const found = await fetchSearch(api, userId, 'a')
    expect(Object.keys(found).sort()).toEqual(
      ['albums', 'artists', 'episodes', 'movies', 'series', 'tracks'].sort(),
    )
    const lyrics = await fetchLyrics(api, albums.items[0]?.Id ?? 'missing')
    console.log('lyrics for first album id (expected null):', lyrics)

    const movies = await fetchItemsPage(
      api,
      userId,
      { includeItemTypes: ['Movie'], pageSize: 1 },
      0,
    )
    const movie = movies.items[0]
    expect(movie?.Id).toBeTruthy()
    const v = await getPlaybackInfo(movie!.Id!, { api, userId })
    console.log(
      'movie',
      movie!.Name,
      v.method,
      v.isHls,
      v.url.replace(/ApiKey=[^&]+/i, 'ApiKey=***'),
    )
    expect(v.url).toMatch(/^https?:\/\//)
    expect([200, 206]).toContain(await firstBytes(v.url))

    const songs = await fetchItemsPage(api, userId, { includeItemTypes: ['Audio'], pageSize: 1 }, 0)
    const song = songs.items[0]
    if (song?.Id) {
      const a = await getPlaybackInfo(song.Id, { api, userId, kind: 'audio' })
      console.log('track', song.Name, a.method, a.url.replace(/ApiKey=[^&]+/i, 'ApiKey=***'))
      expect([200, 206]).toContain(await firstBytes(a.url))
    } else {
      console.log('no tracks in this library')
    }
  })
})
