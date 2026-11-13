import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { DiscordPresence } from '@shared/ipc/discord'
import {
  ACTIVITY_LISTENING,
  ACTIVITY_WATCHING,
  buildActivity,
  isPublicHost,
  safeArtworkUrl,
} from './activity'
import { isDiscordConfigured } from './config'
import { UpdateScheduler } from './scheduler'
import { DiscordService, type RpcClientLike } from './service'

const art = 'https://media.example.com/Items/abc/Images/Primary?maxWidth=512&tag=t1'

const song: DiscordPresence = {
  kind: 'music',
  paused: false,
  title: 'Song',
  artist: 'Artist',
  album: 'Album',
  positionSec: 30,
  durationSec: 200,
  artworkUrl: art,
}

describe('artwork URL rules', () => {
  it('accepts https on a public host', () => {
    expect(safeArtworkUrl(art, true)).toBe(art)
  })
  it('refuses http, localhost, LAN, .local and bare hosts', () => {
    for (const u of [
      'http://media.example.com/a.jpg',
      'https://localhost:8096/a.jpg',
      'https://192.168.1.10/a.jpg',
      'https://10.0.0.2/a.jpg',
      'https://172.20.1.1/a.jpg',
      'https://nas.local/a.jpg',
      'https://jellyfin/a.jpg',
      'https://[::1]/a.jpg',
    ]) {
      expect(safeArtworkUrl(u, true), u).toBeUndefined()
    }
  })
  it('treats public IPs as public', () => {
    expect(isPublicHost('8.8.8.8')).toBe(true)
    expect(isPublicHost('172.32.0.1')).toBe(true)
  })
  it('strips api_key / token parameters and refuses user info', () => {
    const out = safeArtworkUrl(`${art}&api_key=SECRET&X-Emby-Token=SECRET2`, true)
    expect(out).toBe(art)
    expect(safeArtworkUrl('https://u:p@media.example.com/a.jpg', true)).toBeUndefined()
  })
  it('is off when the setting is off', () => {
    expect(safeArtworkUrl(art, false)).toBeUndefined()
  })
})

describe('buildActivity', () => {
  const now = 1_000_000_000

  it('builds a music activity with a progress bar', () => {
    const a = buildActivity(song, { showArtwork: true }, now)!
    expect(a.type).toBe(ACTIVITY_LISTENING)
    expect(a.details).toBe('Song')
    expect(a.state).toBe('Artist — Album')
    expect(a.startTimestamp).toBe(now - 30_000)
    expect(a.endTimestamp).toBe(now - 30_000 + 200_000)
    expect(a.largeImageKey).toBe(art)
    expect(a.largeImageText).toBe('Album')
    expect(a.smallImageKey).toBe('lumen')
    expect(a.largeImageUrl).toBe('https://lumen.dyskolos.fr')
    expect(a.smallImageUrl).toBe('https://lumen.dyskolos.fr')
  })

  it('falls back to the logo without artwork', () => {
    const a = buildActivity(song, { showArtwork: false }, now)!
    expect(a.largeImageKey).toBe('lumen')
  })

  it('builds a movie and an episode', () => {
    const movie = buildActivity(
      {
        kind: 'video',
        paused: false,
        title: 'Dune',
        year: 2021,
        positionSec: 0,
        durationSec: 9000,
      },
      { showArtwork: true },
      now,
    )!
    expect(movie.type).toBe(ACTIVITY_WATCHING)
    expect(movie.details).toBe('Dune')
    expect(movie.state).toBe('2021')

    const ep = buildActivity(
      {
        kind: 'video',
        paused: false,
        title: 'Pilot',
        series: 'Show',
        season: 1,
        episode: 2,
        positionSec: 10,
        durationSec: 2400,
      },
      { showArtwork: true },
      now,
    )!
    expect(ep.details).toBe('Show · S1E2 — Pilot')
    expect(ep.state).toBe('Show')
  })

  it('clears timestamps and shows Paused when paused', () => {
    const a = buildActivity({ ...song, paused: true }, { showArtwork: true }, now)!
    expect(a.startTimestamp).toBeUndefined()
    expect(a.endTimestamp).toBeUndefined()
    expect(a.smallImageKey).toBe('pause')
    expect(a.smallImageText).toBe('Paused')
    expect(a.state).toContain('Paused')
  })

  it('returns null when nothing plays', () => {
    expect(buildActivity(null, { showArtwork: true })).toBeNull()
    expect(buildActivity({ ...song, title: ' ' }, { showArtwork: true })).toBeNull()
  })

  it('never puts a secret in any field', () => {
    const a = buildActivity(
      { ...song, artworkUrl: `${art}&api_key=SECRET` },
      { showArtwork: true },
      now,
    )!
    expect(JSON.stringify(a)).not.toMatch(/api_key|SECRET/i)
  })
})

describe('config', () => {
  it('treats empty and placeholder ids as not configured', () => {
    expect(isDiscordConfigured('')).toBe(false)
    expect(isDiscordConfigured('000000000000000000')).toBe(false)
    expect(isDiscordConfigured('1570792208716537866')).toBe(true)
  })
})

describe('UpdateScheduler (debounce)', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('sends the first update at once and collapses the rest into one trailing update', () => {
    const send = vi.fn()
    const s = new UpdateScheduler<number>(send, 2000, () => Date.now())
    s.push(1)
    expect(send).toHaveBeenCalledTimes(1)
    s.push(2)
    s.push(3)
    expect(send).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(1999)
    expect(send).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(1)
    expect(send).toHaveBeenCalledTimes(2)
    expect(send).toHaveBeenLastCalledWith(3)
  })

  it('never exceeds one update per interval', () => {
    const send = vi.fn()
    const s = new UpdateScheduler<number>(send, 2000, () => Date.now())
    for (let i = 0; i < 20; i++) {
      s.push(i)
      vi.advanceTimersByTime(500)
    }
    vi.advanceTimersByTime(2000)
    // 10 s of pushes -> at most 6 sends.
    expect(send.mock.calls.length).toBeLessThanOrEqual(6)
  })

  it('cancel drops the pending update', () => {
    const send = vi.fn()
    const s = new UpdateScheduler<number>(send, 2000, () => Date.now())
    s.push(1)
    s.push(2)
    s.cancel()
    vi.advanceTimersByTime(5000)
    expect(send).toHaveBeenCalledTimes(1)
  })
})

describe('DiscordService', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  function setup(opts: { configured?: boolean; login?: () => Promise<unknown> } = {}) {
    const client: RpcClientLike = {
      login: vi.fn(opts.login ?? (() => Promise.resolve())),
      setActivity: vi.fn(() => Promise.resolve()),
      clearActivity: vi.fn(() => Promise.resolve()),
      destroy: vi.fn(() => Promise.resolve()),
      on: vi.fn(),
    }
    const createClient = vi.fn(() => client)
    const service = new DiscordService({
      clientId: '1570792208716537866',
      configured: opts.configured ?? true,
      createClient,
      loadSettings: () => ({ enabled: true, showArtwork: true }),
      saveSettings: vi.fn(),
    })
    return { client, createClient, service }
  }

  it('stays inactive with the placeholder id', async () => {
    const { createClient, service } = setup({ configured: false })
    service.setPresence(song)
    await vi.advanceTimersByTimeAsync(5000)
    expect(createClient).not.toHaveBeenCalled()
    expect(service.getState().status).toBe('not-configured')
  })

  it('connects lazily on the first presence and sets the activity', async () => {
    const { client, createClient, service } = setup()
    expect(createClient).not.toHaveBeenCalled()
    service.setPresence(song)
    await vi.advanceTimersByTimeAsync(10)
    expect(client.login).toHaveBeenCalledTimes(1)
    expect(client.setActivity).toHaveBeenCalled()
    expect(service.getState().status).toBe('connected')
  })

  it('stays quiet when Discord is absent and retries every 30 s', async () => {
    let up = false
    const { client, service } = setup({
      login: () => (up ? Promise.resolve() : Promise.reject(new Error('ENOENT'))),
    })
    service.setPresence(song)
    await vi.advanceTimersByTimeAsync(10)
    expect(service.getState().status).toBe('unavailable')
    expect(client.setActivity).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(30_000)
    expect(client.login).toHaveBeenCalledTimes(2)
    up = true
    await vi.advanceTimersByTimeAsync(30_000)
    expect(service.getState().status).toBe('connected')
    expect(client.setActivity).toHaveBeenCalled()
  })

  it('clears the activity at once when the setting is turned off or nothing plays', async () => {
    const { client, service } = setup()
    service.setPresence(song)
    await vi.advanceTimersByTimeAsync(10)
    service.setSettings({ enabled: false })
    await vi.advanceTimersByTimeAsync(0)
    expect(client.clearActivity).toHaveBeenCalledTimes(1)
    expect(service.getState().status).toBe('disabled')
    service.setSettings({ enabled: true })
    await vi.advanceTimersByTimeAsync(10)
    service.setPresence(null)
    await vi.advanceTimersByTimeAsync(0)
    expect(client.clearActivity).toHaveBeenCalledTimes(2)
  })
})
