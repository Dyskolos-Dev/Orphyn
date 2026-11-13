import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { CastDevice, CastMedia, CastSessionState } from '@shared/cast'

vi.mock('@renderer/features/music', async () => {
  const actual = await vi.importActual<typeof import('@renderer/features/music')>(
    '@renderer/features/music',
  )
  return {
    ...actual,
    describeCurrentTrackForCast: () => ({
      url: `http://srv/cast/${actual.musicPlayer.getCurrentTrack()?.Id}`,
      mimeType: 'audio/mpeg',
      kind: 'audio',
      title: 'x',
    }),
  }
})

vi.mock('@renderer/jellyfin', async () => {
  const actual = await vi.importActual<typeof import('@renderer/jellyfin')>('@renderer/jellyfin')
  return {
    ...actual,
    universalAudioUrl: (_api: unknown, _user: string, id: string) => `http://srv/local/${id}`,
    useSession: Object.assign(() => undefined, {
      getState: () => ({ api: {}, account: { userId: 'u' } }),
    }),
  }
})

import { musicPlayer } from '@renderer/features/music'
import { startPlayerEngine } from '@renderer/features/music/player/instance'
import { switchMusicOutput } from './musicCast'

type Ended = (e: { deviceId: string; media: CastMedia | null }) => void
type Sess = (s: CastSessionState) => void

const dev: CastDevice = {
  id: 'chromecast:1',
  name: 'Kitchen',
  kind: 'chromecast',
  model: '',
  manufacturer: '',
  capabilities: { audio: true, video: false },
  address: '10.0.0.3',
  port: 8009,
  wiim: false,
}

describe('music output switching', () => {
  const loads: { media: CastMedia }[] = []
  let ended: Ended[] = []
  let sess: Sess[] = []

  beforeEach(() => {
    loads.length = 0
    ended = []
    sess = []
    ;(window as unknown as { orphyn: unknown }).orphyn = {
      cast: {
        load: vi.fn(async (_id: string, media: CastMedia) => void loads.push({ media })),
        play: vi.fn(async () => undefined),
        pause: vi.fn(async () => undefined),
        seek: vi.fn(async () => undefined),
        setVolume: vi.fn(async () => undefined),
        setMuted: vi.fn(async () => undefined),
        disconnect: vi.fn(async () => undefined),
        onEnded: (cb: Ended) => (ended.push(cb), () => undefined),
        onSessionChanged: (cb: Sess) => (sess.push(cb), () => undefined),
      },
    }
    // Local output is a jsdom <audio>: stub media playback.
    HTMLMediaElement.prototype.play = vi.fn(async () => undefined)
    HTMLMediaElement.prototype.pause = vi.fn()
    HTMLMediaElement.prototype.load = vi.fn()
  })

  it('carries the position to the device, advances the queue on ended, returns paused', async () => {
    startPlayerEngine()
    musicPlayer.play(
      [
        { Id: 't0', Name: 'A', RunTimeTicks: 2e9 },
        { Id: 't1', Name: 'B', RunTimeTicks: 2e9 },
      ],
      0,
    )
    musicPlayer.getOutput() // local
    // Pretend the local output reached 30 s.
    Object.defineProperty(musicPlayer.getOutput(), 'position', { get: () => 30 })

    await switchMusicOutput(dev)
    expect(loads[0]?.media.url).toContain('/t0')
    expect(musicPlayer.getOutput().constructor.name).toBe('RemoteAudioOutput')
    expect(musicPlayer.getOutput().position).toBe(30)

    // The device finishes t0: the queue moves to t1 on the same device.
    ended.forEach((cb) => cb({ deviceId: dev.id, media: loads[0]?.media ?? null }))
    expect(musicPlayer.getCurrentTrack()?.Id).toBe('t1')
    expect(loads[1]?.media.url).toContain('/t1')

    // Remote reports 12 s into t1, then the user disconnects.
    sess.forEach((cb) =>
      cb({
        deviceId: dev.id,
        status: 'playing',
        positionSec: 12,
        durationSec: 200,
        volume: 1,
        muted: false,
        media: loads[1]?.media ?? null,
      }),
    )
    await switchMusicOutput(null)
    expect(musicPlayer.getOutput().constructor.name).toBe('LocalAudioOutput')
    expect(musicPlayer.getState().wantPlaying).toBe(false)
    expect(musicPlayer.getState().startAt).toBe(12)
  })
})
