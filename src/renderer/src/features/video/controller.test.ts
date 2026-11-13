import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { PlaybackInfo } from '@renderer/jellyfin'
import { VideoPlaybackController, type ControllerDeps } from './controller'

function fakeVideo() {
  const el = document.createElement('video')
  let time = 0
  let paused = true
  Object.defineProperty(el, 'currentTime', {
    get: () => time,
    set: (v: number) => {
      time = v
    },
  })
  Object.defineProperty(el, 'paused', { get: () => paused })
  Object.defineProperty(el, 'duration', { get: () => 3600 })
  el.play = vi.fn(() => {
    paused = false
    el.dispatchEvent(new Event('play'))
    el.dispatchEvent(new Event('playing'))
    return Promise.resolve()
  })
  el.pause = vi.fn(() => {
    paused = true
    el.dispatchEvent(new Event('pause'))
  })
  el.load = vi.fn()
  return el
}

const info = (over: Partial<PlaybackInfo> = {}): PlaybackInfo =>
  ({
    itemId: 'm1',
    url: 'http://srv/video.mp4',
    isHls: false,
    method: 'DirectPlay',
    playSessionId: 'ps',
    mediaSource: {},
    mediaSourceId: 'ms',
    tracks: { audio: [], subtitles: [] },
    startTimeTicks: 0,
    kind: 'video',
    ...over,
  }) as PlaybackInfo

function setup(first: PlaybackInfo = info()) {
  const reporter = {
    start: vi.fn(() => Promise.resolve()),
    stop: vi.fn(() => Promise.resolve()),
    pause: vi.fn(() => Promise.resolve()),
    resume: vi.fn(() => Promise.resolve()),
    tick: vi.fn(() => Promise.resolve()),
  }
  const deps: ControllerDeps = {
    getPlaybackInfo: vi.fn(() => Promise.resolve(first)),
    createReporter: vi.fn(() => reporter),
    createHls: vi.fn(() => null),
    transcodeFallback: vi.fn((i) => ({
      ...i,
      url: 'http://srv/master.m3u8',
      isHls: true,
      method: 'Transcode' as const,
    })),
    getMaxBitrate: () => undefined,
    announceExclusive: vi.fn(),
  }
  const video = fakeVideo()
  const c = new VideoPlaybackController(deps)
  c.attach(video)
  return { c, deps, video, reporter }
}

describe('VideoPlaybackController', () => {
  let ctx: ReturnType<typeof setup>
  beforeEach(() => {
    ctx = setup()
  })

  it('plays an item: announces exclusivity, requests info at the start ticks and resumes', async () => {
    const { c, deps, video, reporter } = ctx
    await c.play('m1', 600_000_000)
    expect(deps.announceExclusive).toHaveBeenCalled()
    expect(deps.getPlaybackInfo).toHaveBeenCalledWith(
      'm1',
      expect.objectContaining({ startTimeTicks: 600_000_000 }),
    )
    expect(video.src).toContain('video.mp4')
    video.dispatchEvent(new Event('loadedmetadata'))
    expect(video.currentTime).toBe(60)
    expect(video.play).toHaveBeenCalled()
    expect(c.getSnapshot()).toMatchObject({ itemId: 'm1', status: 'playing', method: 'DirectPlay' })
    expect(reporter.start).toHaveBeenCalledTimes(1)
  })

  it('pauses, seeks and notifies subscribers', async () => {
    const { c, video, reporter } = ctx
    const listener = vi.fn()
    c.subscribe(listener)
    await c.play('m1')
    video.dispatchEvent(new Event('loadedmetadata'))
    c.pause()
    expect(c.getSnapshot().status).toBe('paused')
    expect(reporter.pause).toHaveBeenCalled()
    c.seek(120)
    expect(video.currentTime).toBe(120)
    expect(c.getSnapshot().positionSeconds).toBe(120)
    c.seek(99999)
    expect(video.currentTime).toBe(3600)
    expect(listener).toHaveBeenCalled()
  })

  it('stop reports to the server and resets the state', async () => {
    const { c, reporter } = ctx
    await c.play('m1')
    await c.stop()
    expect(reporter.stop).toHaveBeenCalled()
    expect(c.getSnapshot()).toMatchObject({ itemId: null, status: 'idle' })
  })

  it('falls back to a transcoded stream when direct play fails, then reports the error', async () => {
    const { c, deps, video } = ctx
    await c.play('m1')
    video.dispatchEvent(new Event('error'))
    expect(deps.transcodeFallback).toHaveBeenCalledTimes(1)
    expect(c.getSnapshot().method).toBe('Transcode')
    expect(c.getSnapshot().status).not.toBe('error')
    // hls unsupported in this fake: createHls returned null, so the url is set as a plain source
    video.dispatchEvent(new Event('error'))
    expect(c.getSnapshot().status).toBe('error')
  })

  it('surfaces a playback info failure', async () => {
    const { c, deps } = ctx
    vi.mocked(deps.getPlaybackInfo).mockRejectedValueOnce(new Error('no stream'))
    await c.play('m1')
    expect(c.getSnapshot()).toMatchObject({ status: 'error', error: 'no stream' })
  })

  it('switching tracks re-requests playback at the current position', async () => {
    const { c, deps, video } = ctx
    await c.play('m1')
    video.currentTime = 90
    await c.switchTracks({ audioIndex: 2 })
    expect(deps.getPlaybackInfo).toHaveBeenLastCalledWith(
      'm1',
      expect.objectContaining({ startTimeTicks: 900_000_000, audioStreamIndex: 2 }),
    )
  })

  it('ignores a stale response when a newer play started', async () => {
    const { c, deps } = ctx
    let release!: (i: PlaybackInfo) => void
    vi.mocked(deps.getPlaybackInfo).mockImplementationOnce(() => new Promise((r) => (release = r)))
    const first = c.play('old')
    await c.play('m1')
    release(info({ itemId: 'old' }))
    await first
    expect(c.getSnapshot().itemId).toBe('m1')
  })
})
