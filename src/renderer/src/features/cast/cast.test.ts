import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { CastDevice, CastMedia, CastSessionState } from '@shared/cast'
import type { CastApi } from '@shared/ipc'
import { devicesForScope, groupDevices } from './devices'
import { RemoteAudioOutput } from './RemoteAudioOutput'
import type { OutputEvent } from '@renderer/features/music'

const device = (over: Partial<CastDevice> & { id: string }): CastDevice => ({
  name: over.id,
  kind: 'chromecast',
  model: '',
  manufacturer: '',
  capabilities: { audio: true, video: true },
  address: '10.0.0.2',
  port: 8009,
  wiim: false,
  ...over,
})

function fakeCast() {
  const sessionCbs = new Set<(s: CastSessionState) => void>()
  const endedCbs = new Set<(e: { deviceId: string; media: CastMedia | null }) => void>()
  const calls: string[] = []
  const loads: { deviceId: string; media: CastMedia }[] = []
  const api = {
    load: vi.fn(async (deviceId: string, media: CastMedia) => {
      calls.push('load')
      loads.push({ deviceId, media })
    }),
    play: vi.fn(async () => void calls.push('play')),
    pause: vi.fn(async () => void calls.push('pause')),
    seek: vi.fn(async () => void calls.push('seek')),
    setVolume: vi.fn(async () => undefined),
    setMuted: vi.fn(async () => undefined),
    disconnect: vi.fn(async () => undefined),
    onSessionChanged: (cb: (s: CastSessionState) => void) => {
      sessionCbs.add(cb)
      return () => sessionCbs.delete(cb)
    },
    onEnded: (cb: (e: { deviceId: string; media: CastMedia | null }) => void) => {
      endedCbs.add(cb)
      return () => endedCbs.delete(cb)
    },
  } as unknown as CastApi
  return {
    api,
    loads,
    calls,
    session: (s: Partial<CastSessionState> & { deviceId: string }) =>
      sessionCbs.forEach((cb) =>
        cb({
          status: 'playing',
          positionSec: 0,
          durationSec: 0,
          volume: 1,
          muted: false,
          media: null,
          ...s,
        }),
      ),
    ended: (deviceId: string, media: CastMedia | null) =>
      endedCbs.forEach((cb) => cb({ deviceId, media })),
  }
}

const media = (url: string): CastMedia => ({
  url,
  mimeType: 'audio/mpeg',
  kind: 'audio',
  title: url,
})

describe('device filtering by capability', () => {
  const devices = [
    device({ id: 'tv' }),
    device({ id: 'wiim', kind: 'upnp', wiim: true, capabilities: { audio: true, video: false } }),
    device({ id: 'homepod', kind: 'airplay', capabilities: { audio: false, video: false } }),
  ]
  it('lists only video-capable devices for video', () => {
    expect(devicesForScope(devices, 'video').map((d) => d.id)).toEqual(['tv'])
  })
  it('lists audio-capable devices for music and hides unsupported ones', () => {
    expect(devicesForScope(devices, 'music').map((d) => d.id)).toEqual(['tv', 'wiim'])
  })
  it('shows nothing without any device', () => {
    expect(devicesForScope([], 'music')).toEqual([])
    expect(groupDevices([])).toEqual([])
  })
  it('groups speakers and TVs', () => {
    expect(groupDevices(devices.slice(0, 2)).map((g) => g.group)).toEqual(['speaker', 'tv'])
  })
})

describe('RemoteAudioOutput', () => {
  let cast: ReturnType<typeof fakeCast>
  let out: RemoteAudioOutput
  let events: OutputEvent[]
  beforeEach(() => {
    cast = fakeCast()
    events = []
    out = new RemoteAudioOutput({ cast: cast.api, deviceId: 'd1', describe: (src) => media(src) })
    out.subscribe((e) => events.push(e))
  })

  it('loads at the carried-over position and follows session events', () => {
    out.load('http://s/a', { startAt: 42, autoplay: true })
    expect(cast.loads[0]?.media).toMatchObject({ url: 'http://s/a', startPositionSec: 42 })
    expect(out.position).toBe(42)
    cast.session({ deviceId: 'd1', positionSec: 43, durationSec: 200, media: media('http://s/a') })
    expect(out.position).toBe(43)
    expect(out.duration).toBe(200)
    expect(out.paused).toBe(false)
    expect(events).toContainEqual({ type: 'state', state: 'playing' })
  })

  it('pauses after loading when autoplay is off (return to local paused)', async () => {
    out.load('http://s/a', { startAt: 10, autoplay: false })
    await Promise.resolve()
    await Promise.resolve()
    expect(cast.calls).toEqual(['load', 'pause'])
    expect(out.paused).toBe(true)
  })

  it('emits ended once for the current track and ignores previous ones', () => {
    out.load('http://s/b')
    cast.ended('d1', media('http://s/a'))
    cast.ended('other', media('http://s/b'))
    expect(events.filter((e) => e.type === 'ended')).toHaveLength(0)
    cast.ended('d1', media('http://s/b'))
    expect(events.filter((e) => e.type === 'ended')).toHaveLength(1)
  })

  it('ignores session state of the previous media while a load is in flight', () => {
    out.load('http://s/b', { startAt: 0 })
    cast.session({ deviceId: 'd1', positionSec: 199, media: media('http://s/a') })
    expect(out.position).toBe(0)
  })

  it('hands device failures to onFailure instead of the engine', () => {
    const onFailure = vi.fn()
    const o = new RemoteAudioOutput({ cast: cast.api, deviceId: 'd1', describe: media2, onFailure })
    o.load('http://s/a')
    cast.session({ deviceId: 'd1', status: 'error', error: 'gone', media: media('http://s/a') })
    expect(onFailure).toHaveBeenCalledWith('gone')
  })
})

function media2(src: string): CastMedia {
  return media(src)
}
