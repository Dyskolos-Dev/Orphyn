// @vitest-environment node
/**
 * Opt-in check against a real Jellyfin server: two independent SyncPlay members (two module
 * instances of the manager, two devices) with simulated players, through create / join / play /
 * pause / seek / drift / leave. Skipped unless ORPHYN_LIVE_URL is set:
 *
 *   ORPHYN_LIVE_URL=https://server ORPHYN_LIVE_USER=user ORPHYN_LIVE_PASSWORD=… \
 *     npx vitest run src/renderer/src/features/syncplay/syncplay.live.test.ts
 */
import { describe, expect, it, vi } from 'vitest'
import type { SyncPlayer } from './types'

// tsconfig.web has no node types: read the environment through globalThis.
const env: Record<string, string | undefined> =
  (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env ?? {}
const URL = env['ORPHYN_LIVE_URL']
const USER = env['ORPHYN_LIVE_USER'] ?? ''
const PASSWORD = env['ORPHYN_LIVE_PASSWORD'] ?? ''

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

/** A player whose clock runs in real time at its rate; loading takes `loadMs`. */
class SimPlayer implements SyncPlayer {
  readonly kind = 'video' as const
  private id: string | null = null
  private base = 0
  private baseAt = Date.now()
  private playing = false
  private rate = 1
  private readyAt = 0
  private readonly listeners = new Set<() => void>()
  rates: number[] = []
  constructor(private readonly loadMs = 300) {}
  load(id: string, at: number) {
    this.id = id
    this.base = at
    this.baseAt = Date.now()
    this.readyAt = Date.now() + this.loadMs
    this.playing = false
    this.emit()
    setTimeout(() => this.emit(), this.loadMs + 10)
  }
  itemId = () => this.id
  position = () => this.base + (this.playing ? ((Date.now() - this.baseAt) / 1000) * this.rate : 0)
  isPlaying = () => this.playing
  isReady = () => this.id !== null && Date.now() >= this.readyAt
  play = () => {
    this.rebase()
    this.playing = true
    this.emit()
  }
  pause = () => {
    this.rebase()
    this.playing = false
    this.emit()
  }
  seek = (s: number) => {
    this.base = s
    this.baseAt = Date.now()
    this.readyAt = Date.now() + 150
    this.emit()
    setTimeout(() => this.emit(), 160)
  }
  setRate = (r: number) => {
    this.rebase()
    this.rate = r
    this.rates.push(r)
  }
  /** Simulates a hiccup: jumps ahead without telling anyone. */
  jump(by: number) {
    this.rebase()
    this.base += by
  }
  subscribe = (l: () => void) => {
    this.listeners.add(l)
    return () => this.listeners.delete(l)
  }
  private rebase() {
    this.base = this.position()
    this.baseAt = Date.now()
  }
  private emit() {
    this.listeners.forEach((l) => l())
  }
}

async function login(base: string, device: string) {
  const auth = `MediaBrowser Client="Orphyn", Device="live-test", DeviceId="${device}", Version="1.3.0"`
  const res = await fetch(`${base}/Users/AuthenticateByName`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: auth },
    body: JSON.stringify({ Username: USER, Pw: PASSWORD }),
  })
  if (!res.ok) throw new Error(`login ${res.status}`)
  const j = (await res.json()) as { AccessToken: string; User: { Id: string; Name: string } }
  return { token: j.AccessToken, userId: j.User.Id, userName: j.User.Name, auth }
}

/** One member: its own copy of the data layer and of the manager. */
async function member(base: string, device: string) {
  vi.resetModules()
  const jf = await import('@renderer/jellyfin')
  const client = await import('@renderer/jellyfin/client')
  const mod = await import('./manager')
  const who = await login(base, device)
  const api = client.createJellyfinApi({ baseUrl: base, deviceId: device, accessToken: who.token })
  jf.useSession.setState({
    status: 'ready',
    api,
    deviceId: device,
    account: {
      id: device,
      name: 'live',
      url: base,
      serverId: '',
      serverName: '',
      version: '',
      userId: who.userId,
      userName: who.userName,
      addedAt: '',
      lastUsedAt: '',
      hasToken: true,
    },
  })
  const manager = new mod.SyncPlayManager()
  const player = new SimPlayer()
  const notices: string[] = []
  manager.onNotice((n) => notices.push(n.kind))
  manager.setNavigator((path) => {
    const m = /\/video\/play\/([^?]+)\?start=(\d+)/.exec(path)
    if (m) player.load(jf.syncPlay.normalizeGuid(m[1]), Number(m[2]) / 1e7)
  })
  return { manager, player, notices, store: mod.useSyncPlay, who, api, jf }
}

async function until(check: () => boolean, ms = 8000) {
  const end = Date.now() + ms
  while (Date.now() < end) {
    if (check()) return
    await sleep(50)
  }
  throw new Error('timed out')
}

describe.skipIf(!URL)('SyncPlay against a real server (two members)', () => {
  it('keeps two members in step', { timeout: 90_000 }, async () => {
    const base = URL!.replace(/\/+$/, '')
    const a = await member(base, `orphyn-live-a-${Date.now()}`)
    const b = await member(base, `orphyn-live-b-${Date.now()}`)

    // A video the user can see.
    const res = await fetch(
      `${base}/Items?userId=${a.who.userId}&Recursive=true&IncludeItemTypes=Movie&Limit=1`,
      { headers: { Authorization: `${a.who.auth}, Token="${a.who.token}"` } },
    )
    const movie = ((await res.json()) as { Items: { Id: string }[] }).Items[0]
    expect(movie).toBeTruthy()
    const movieId = a.jf.syncPlay.normalizeGuid(movie!.Id)

    // A opens the film, then creates a room: the film becomes the room's and starts.
    a.player.load(movieId, 10)
    const detachA = a.manager.attachVideo(a.player)
    await a.manager.create('Live test room')
    await until(() => a.store.getState().phase === 'joined')
    await until(() => a.player.isPlaying())

    // B joins: it is sent to the film at the room's position; the room waits, then both play.
    const groupId = a.store.getState().group!.id
    expect((await b.manager.listGroups()).some((g) => g.id === groupId)).toBe(true)
    const detachB = b.manager.attachVideo(b.player)
    await b.manager.join(groupId)
    await until(() => b.player.itemId() === movieId)
    await until(() => a.player.isPlaying() && b.player.isPlaying(), 12_000)
    await until(() => a.notices.includes('userJoined'))
    await sleep(2500)
    expect(Math.abs(a.player.position() - b.player.position())).toBeLessThan(0.1)

    // B pauses: both stop on the same frame; A is told.
    b.manager.requestPause()
    await until(() => !a.player.isPlaying() && !b.player.isPlaying())
    await sleep(500)
    expect(Math.abs(a.player.position() - b.player.position())).toBeLessThan(0.05)
    await until(() => a.notices.includes('paused'))

    // A seeks while paused, then resumes: both continue from there together.
    a.manager.requestSeek(120)
    await until(() => Math.abs(b.player.position() - 120) < 0.05)
    a.manager.requestPlay()
    await until(() => a.player.isPlaying() && b.player.isPlaying())
    await sleep(1500)
    expect(Math.abs(a.player.position() - b.player.position())).toBeLessThan(0.1)
    expect(a.player.position()).toBeGreaterThan(120)

    // B drifts half a second ahead: it slows down until it is back in step (no seek).
    b.player.jump(0.5)
    await until(() => b.player.rates.some((r) => r < 1))
    await until(() => Math.abs(a.player.position() - b.player.position()) < 0.06, 20_000)

    // B's connection drops: the server takes it out of the room; B rejoins by itself and the
    // room picks it up again, in step.
    const internals = b.manager as unknown as { socket: { ws: { close(): void } | null } }
    internals.socket.ws?.close()
    await until(() => b.notices.includes('connectionLost'))
    await until(() => b.notices.includes('reconnected'), 20_000)
    await until(() => a.player.isPlaying() && b.player.isPlaying(), 15_000)
    await sleep(2000)
    expect(Math.abs(a.player.position() - b.player.position())).toBeLessThan(0.1)

    // B leaves mid-playback: A keeps playing, is told; B keeps playing on its own.
    await b.manager.leave()
    await until(() => a.notices.includes('userLeft'))
    expect(a.player.isPlaying()).toBe(true)
    expect(b.store.getState().phase).toBe('off')

    detachA()
    detachB()
    await a.manager.leave()
  })
})
