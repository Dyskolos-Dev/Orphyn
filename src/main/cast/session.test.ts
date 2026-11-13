// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CastMedia, CastSessionState } from '@shared/cast'
import { CastManager } from './manager'
import { DeviceRegistry } from './registry'
import { CastSession } from './session'
import type { CastTransport, TransportHandlers } from './transport'
import type { SoapCallOptions } from './soap'
import { UpnpTransport, type SoapFn } from './transports/upnp'

const media: CastMedia = {
  url: 'http://h/a.mp3',
  mimeType: 'audio/mpeg',
  kind: 'audio',
  title: 'T',
}

class FakeTransport implements CastTransport {
  handlers!: TransportHandlers
  calls: string[] = []
  failConnect = false
  failLoad = false
  setHandlers(h: TransportHandlers): void {
    this.handlers = h
  }
  async connect(): Promise<void> {
    this.calls.push('connect')
    if (this.failConnect) throw new Error('unreachable')
  }
  async load(m: CastMedia): Promise<void> {
    this.calls.push(`load:${m.startPositionSec ?? 0}`)
    if (this.failLoad) throw new Error('refused')
  }
  async play(): Promise<void> {
    this.calls.push('play')
  }
  async pause(): Promise<void> {
    this.calls.push('pause')
  }
  async seek(s: number): Promise<void> {
    this.calls.push(`seek:${s}`)
  }
  async stop(): Promise<void> {
    this.calls.push('stop')
  }
  async setVolume(v: number): Promise<void> {
    this.calls.push(`vol:${v}`)
  }
  async setMuted(m: boolean): Promise<void> {
    this.calls.push(`mute:${m}`)
  }
  async close(): Promise<void> {
    this.calls.push('close')
  }
}

function setup(opts: { reconnectDelaysMs?: number[] } = {}) {
  const transports: FakeTransport[] = []
  const behavior = { failConnect: false, failLoad: false }
  const states: CastSessionState[] = []
  const ended: Array<CastMedia | null> = []
  const errors: string[] = []
  const session = new CastSession(
    'dev',
    () => {
      const t = new FakeTransport()
      t.failConnect = behavior.failConnect
      t.failLoad = behavior.failLoad
      transports.push(t)
      return t
    },
    {
      onState: (s) => states.push(s),
      onEnded: (_id, m) => ended.push(m),
      onError: (_id, msg) => errors.push(msg),
    },
    { reconnectDelaysMs: [10, 20], ...opts },
  )
  return { session, transports, states, ended, errors, behavior }
}

describe('CastSession state machine', () => {
  it('goes idle -> connecting -> buffering -> playing -> paused', async () => {
    const { session, transports, states } = setup()
    await session.load(media)
    expect(states.map((s) => s.status)).toEqual(['connecting', 'buffering'])
    transports[0]!.handlers.onUpdate({
      status: 'playing',
      positionSec: 3,
      durationSec: 100,
      volume: 0.4,
    })
    expect(session.getState()).toMatchObject({
      status: 'playing',
      positionSec: 3,
      durationSec: 100,
      volume: 0.4,
    })
    await session.pause()
    expect(session.getState().status).toBe('paused')
    await session.seek(42)
    expect(session.getState().positionSec).toBe(42)
    await session.setVolume(2)
    expect(session.getState().volume).toBe(1) // clamped
    await session.setMuted(true)
    expect(session.getState().muted).toBe(true)
    expect(transports[0]!.calls).toEqual([
      'connect',
      'load:0',
      'pause',
      'seek:42',
      'vol:1',
      'mute:true',
    ])
  })

  it('emits ended exactly once and returns to idle', async () => {
    const { session, transports, ended } = setup()
    await session.load(media)
    const t = transports[0]!
    t.handlers.onUpdate({ status: 'playing' })
    t.handlers.onUpdate({ ended: true })
    t.handlers.onUpdate({ ended: true })
    expect(ended).toEqual([media])
    expect(session.getState()).toMatchObject({ status: 'idle', media: null })
  })

  it('does not emit ended or revive the session after a user stop', async () => {
    const { session, transports, ended } = setup()
    await session.load(media)
    await session.stop()
    transports[0]!.handlers.onUpdate({ status: 'playing' })
    transports[0]!.handlers.onUpdate({ ended: true })
    expect(ended).toHaveLength(0)
    expect(session.getState().status).toBe('idle')
  })

  it('turns a failed connect or load into an error state and a rejected promise', async () => {
    const a = setup()
    a.behavior.failConnect = true
    await expect(a.session.load(media)).rejects.toThrow('unreachable')
    expect(a.session.getState()).toMatchObject({ status: 'error', error: 'unreachable' })
    expect(a.errors).toEqual(['unreachable'])

    const b = setup()
    b.behavior.failLoad = true
    await expect(b.session.load(media)).rejects.toThrow('refused')
    expect(b.session.getState().status).toBe('error')
  })

  it('reports a device playback error', async () => {
    const { session, transports, errors } = setup()
    await session.load(media)
    transports[0]!.handlers.onUpdate({ error: 'bad media' })
    expect(session.getState()).toMatchObject({ status: 'error', error: 'bad media' })
    expect(errors).toContain('bad media')
  })

  it('rejects commands when not connected without throwing synchronously', async () => {
    const { session } = setup()
    await expect(session.play()).rejects.toThrow(/Not connected/)
  })

  it('keeps the queue usable after a failing command', async () => {
    const { session, transports } = setup()
    await session.load(media)
    transports[0]!.pause = () => Promise.reject(new Error('boom'))
    await expect(session.pause()).rejects.toThrow('boom')
    await session.play()
    expect(transports[0]!.calls).toContain('play')
  })
})

describe('CastSession reconnection', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('reconnects when the link drops mid-playback and resumes at the last position', async () => {
    const { session, transports } = setup()
    await session.load(media)
    transports[0]!.handlers.onUpdate({ status: 'playing', positionSec: 30 })
    transports[0]!.handlers.onClosed(new Error('socket closed'))
    expect(session.getState().status).toBe('connecting')
    await vi.advanceTimersByTimeAsync(15)
    expect(transports).toHaveLength(2)
    expect(transports[1]!.calls).toEqual(['connect', 'load:30'])
    expect(session.getState().status).toBe('buffering')
  })

  it('gives up with an error when the device vanished for good, without unhandled rejections', async () => {
    const { session, transports, errors, behavior } = setup()
    await session.load(media)
    transports[0]!.handlers.onUpdate({ status: 'playing', positionSec: 5 })
    behavior.failConnect = true
    transports[0]!.handlers.onClosed(new Error('gone'))
    await vi.advanceTimersByTimeAsync(100)
    expect(session.getState()).toMatchObject({ status: 'error', error: 'Device unreachable' })
    expect(errors).toEqual(['Device unreachable'])
  })

  it('goes quietly idle when an idle session loses its link', async () => {
    const { session, transports, errors } = setup()
    await session.load(media)
    await session.stop()
    transports[0]!.handlers.onClosed(new Error('gone'))
    expect(session.getState().status).toBe('idle')
    expect(errors).toHaveLength(0)
  })

  it('disconnect cancels a pending reconnect', async () => {
    const { session, transports } = setup()
    await session.load(media)
    transports[0]!.handlers.onUpdate({ status: 'playing' })
    transports[0]!.handlers.onClosed()
    await session.disconnect()
    await vi.advanceTimersByTimeAsync(100)
    expect(transports).toHaveLength(1)
    expect(session.getState().status).toBe('idle')
  })
})

describe('UpnpTransport polling (mocked SOAP)', () => {
  function make(states: string[]) {
    const seen: string[] = []
    const updates: Array<Record<string, unknown>> = []
    let i = 0
    const soap = vi.fn(async (o: SoapCallOptions) => {
      seen.push(o.action)
      if (o.action === 'GetTransportInfo')
        return { CurrentTransportState: states[Math.min(i++, states.length - 1)]! }
      if (o.action === 'GetPositionInfo') return { RelTime: '0:00:07', TrackDuration: '0:03:00' }
      return {}
    }) as unknown as ReturnType<typeof vi.fn<SoapFn>>
    const t = new UpnpTransport(
      { avTransportControlUrl: 'http://x/av' },
      { soap, maxPollFailures: 2 },
    )
    t.setHandlers({
      onUpdate: (u) => updates.push(u as Record<string, unknown>),
      onClosed: () => updates.push({ closed: true }),
    })
    return { t, seen, updates, soap }
  }

  it('sends SetAVTransportURI with DIDL then Play on load', async () => {
    const { t, soap } = make(['PLAYING'])
    await t.load(media)
    await t.close()
    const actions = soap.mock.calls.map((c) => c[0].action)
    expect(actions).toEqual(['Stop', 'SetAVTransportURI', 'Play'])
    const set = soap.mock.calls[1]![0] as { args: Array<[string, string]> }
    expect(set.args.find(([k]) => k === 'CurrentURIMetaData')![1]).toContain('<DIDL-Lite')
  })

  it('reports ended only when STOPPED follows PLAYING', async () => {
    const { t, updates } = make(['STOPPED', 'PLAYING', 'STOPPED'])
    await t.poll() // STOPPED before ever playing: initial state, not an end
    expect(updates.some((u) => u.ended)).toBe(false)
    await t.poll() // PLAYING
    expect(updates.at(-1)).toMatchObject({ status: 'playing', positionSec: 7, durationSec: 180 })
    await t.poll() // STOPPED after playing
    expect(updates.at(-1)).toEqual({ ended: true })
    await t.poll()
    expect(updates.filter((u) => u.ended)).toHaveLength(1)
  })

  it('does not report ended after a user stop', async () => {
    const { t, updates } = make(['PLAYING', 'STOPPED'])
    await t.poll()
    await t.stop()
    await t.poll()
    expect(updates.some((u) => u.ended)).toBe(false)
  })

  it('reports a closed link after repeated poll failures', async () => {
    const updates: Array<Record<string, unknown>> = []
    const soap = vi.fn<SoapFn>(async () => {
      throw new Error('ECONNREFUSED')
    })
    const t = new UpnpTransport(
      { avTransportControlUrl: 'http://x/av' },
      { soap, maxPollFailures: 2 },
    )
    t.setHandlers({ onUpdate: () => undefined, onClosed: () => updates.push({ closed: true }) })
    await t.poll()
    expect(updates).toHaveLength(0)
    await t.poll()
    expect(updates).toEqual([{ closed: true }])
  })

  it('maps volume to 0..100 and mute to 0/1', async () => {
    const soap = vi.fn<SoapFn>(async () => ({}))
    const t = new UpnpTransport(
      { avTransportControlUrl: 'http://x/av', renderingControlUrl: 'http://x/rc' },
      { soap },
    )
    await t.setVolume(0.5)
    await t.setMuted(true)
    expect(soap.mock.calls[0]![0].args).toEqual([
      ['InstanceID', 0],
      ['Channel', 'Master'],
      ['DesiredVolume', 50],
    ])
    expect(soap.mock.calls[1]![0].args).toEqual([
      ['InstanceID', 0],
      ['Channel', 'Master'],
      ['DesiredMute', 1],
    ])
  })
})

describe('DeviceRegistry', () => {
  const dev = (id: string, name = id) => ({
    id,
    name,
    kind: 'upnp' as const,
    model: '',
    manufacturer: '',
    capabilities: { audio: true, video: false },
    address: '1.1.1.1',
    port: 80,
    wiim: false,
  })

  it('notifies on add/change/remove and expires stale devices', () => {
    const changes: string[][] = []
    const reg = new DeviceRegistry((d) => changes.push(d.map((x) => x.id)), 1000)
    reg.upsert(dev('a'), 0)
    reg.upsert(dev('b'), 500)
    reg.upsert(dev('a'), 600) // unchanged: no event
    expect(changes).toEqual([['a'], ['a', 'b']])
    reg.sweep(1200) // a seen at 600 -> alive; none expired
    expect(changes).toHaveLength(2)
    reg.sweep(1550) // b (seen 500) expired, a (seen 600) alive
    expect(changes.at(-1)).toEqual(['a'])
    reg.remove('a')
    expect(changes.at(-1)).toEqual([])
  })
})

describe('CastManager', () => {
  it('works with no devices: commands reject, nothing throws, dispose is clean', async () => {
    const start = vi.fn()
    const stop = vi.fn()
    const m = new CastManager({
      discovery: { start, stop },
      listDevices: () => [],
      getDevice: () => undefined,
      createTransport: () => null,
      events: {
        onDevicesChanged: vi.fn(),
        onSessionChanged: vi.fn(),
        onEnded: vi.fn(),
        onError: vi.fn(),
      },
    })
    expect(m.listDevices()).toEqual([])
    await expect(m.load('nope', media)).rejects.toThrow(/not available/)
    await expect(m.play('nope')).rejects.toThrow(/No active session/)
    await m.disconnect('nope')
    await m.dispose()
    expect(stop).toHaveBeenCalled()
    await expect(m.load('nope', media)).rejects.toThrow()
  })

  it('runs discovery only while asked, after a short startup scan', async () => {
    vi.useFakeTimers()
    try {
      const start = vi.fn()
      const stop = vi.fn()
      const m = new CastManager({
        discovery: { start, stop },
        listDevices: () => [],
        getDevice: () => undefined,
        createTransport: () => null,
        events: {
          onDevicesChanged: vi.fn(),
          onSessionChanged: vi.fn(),
          onEnded: vi.fn(),
          onError: vi.fn(),
        },
      })
      m.startupScan(1000)
      expect(start).toHaveBeenCalledTimes(1)
      await vi.advanceTimersByTimeAsync(1000)
      expect(stop).toHaveBeenCalledTimes(1)
      m.startDiscovery()
      m.stopDiscovery()
      expect(stop).toHaveBeenCalledTimes(2)
      // Renderer asks during the startup scan: keeps running after the scan ends.
      m.startupScan(1000)
      m.startDiscovery()
      await vi.advanceTimersByTimeAsync(1000)
      expect(stop).toHaveBeenCalledTimes(2)
      await m.dispose()
    } finally {
      vi.useRealTimers()
    }
  })

  it('rejects video on audio-only devices and unsupported devices', async () => {
    const d = {
      id: 'upnp:x',
      name: 'WiiM',
      kind: 'upnp' as const,
      model: '',
      manufacturer: '',
      capabilities: { audio: true, video: false },
      address: '1.1.1.1',
      port: 80,
      wiim: true,
    }
    const m = new CastManager({
      discovery: { start: vi.fn(), stop: vi.fn() },
      listDevices: () => [d],
      getDevice: () => d,
      createTransport: () => new FakeTransport(),
      events: {
        onDevicesChanged: vi.fn(),
        onSessionChanged: vi.fn(),
        onEnded: vi.fn(),
        onError: vi.fn(),
      },
    })
    await expect(m.load(d.id, { ...media, kind: 'video' })).rejects.toThrow(/cannot play video/)
    await m.load(d.id, media)
    expect(m.getSession(d.id)?.status).toBe('buffering')
    await m.dispose()
  })
})
