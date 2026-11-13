import { afterEach, describe, expect, it, vi } from 'vitest'
import { createJellyfinApi } from '../client'
import { fakeHttp, networkError } from '../test-utils'
import { fetchLyrics } from '../endpoints'
import {
  PlaybackError,
  getPlaybackInfo,
  streamUrlForCast,
  subtitleUrl,
  universalAudioUrl,
} from './info'
import { PlaybackReporter, reportPlaying, type ReportTarget } from './reporting'

const mk = (handler: Parameters<typeof fakeHttp>[0]) => {
  const f = fakeHttp(handler)
  const api = createJellyfinApi({
    baseUrl: 'http://s:8096/jellyfin',
    deviceId: 'dev1',
    accessToken: 'TOK',
    http: f.http,
  })
  return { ...f, api }
}

const movieSource = {
  Id: 'src1',
  ETag: 'etag',
  SupportsDirectPlay: true,
  MediaStreams: [
    { Index: 0, Type: 'Video', Codec: 'h264' },
    { Index: 1, Type: 'Audio', Codec: 'aac', Language: 'eng', Channels: 2, IsDefault: true },
    { Index: 2, Type: 'Audio', Codec: 'ac3', Language: 'fra', Channels: 6 },
    { Index: 3, Type: 'Subtitle', Codec: 'subrip', Language: 'eng', IsTextSubtitleStream: true },
    { Index: 4, Type: 'Subtitle', Codec: 'PGSSUB', Language: 'fra', IsTextSubtitleStream: false },
  ],
  DefaultAudioStreamIndex: 1,
}

describe('getPlaybackInfo', () => {
  it('direct play: static stream URL with ApiKey, tracks and VTT subtitle URLs', async () => {
    const { api, calls } = mk(() => ({
      data: { PlaySessionId: 'ps1', MediaSources: [movieSource] },
    }))
    const info = await getPlaybackInfo('m1', {
      api,
      userId: 'u1',
      startTimeTicks: 600_000_000,
      audioStreamIndex: 2,
      subtitleStreamIndex: 3,
      maxBitrate: 8_000_000,
    })
    expect(info).toMatchObject({
      method: 'DirectPlay',
      isHls: false,
      playSessionId: 'ps1',
      mediaSourceId: 'src1',
      kind: 'video',
    })
    const u = new URL(info.url)
    expect(u.origin + u.pathname).toBe('http://s:8096/jellyfin/Videos/m1/stream')
    expect(u.searchParams.get('static')).toBe('true')
    expect(u.searchParams.get('ApiKey')).toBe('TOK')
    expect(u.searchParams.get('mediaSourceId')).toBe('src1')
    expect(info.tracks.audio.map((a) => a.index)).toEqual([1, 2])
    expect(info.tracks.subtitles[0]?.vttUrl).toBe(
      'http://s:8096/jellyfin/Videos/m1/src1/Subtitles/3/0/Stream.vtt?ApiKey=TOK',
    )
    expect(info.tracks.subtitles[1]).toMatchObject({ burnInRequired: true, vttUrl: undefined })
    expect(info.tracks.defaultAudioIndex).toBe(1)

    const req = calls[0]!
    expect(req.method).toBe('POST')
    expect(req.url).toContain('/Items/m1/PlaybackInfo?')
    for (const p of [
      'userId=u1',
      'startTimeTicks=600000000',
      'audioStreamIndex=2',
      'subtitleStreamIndex=3',
      'maxStreamingBitrate=8000000',
    ]) {
      expect(req.url).toContain(p)
    }
    expect((req.data as { DeviceProfile: { Name: string } }).DeviceProfile.Name).toMatch(/Orphyn/)
  })

  it('transcode: relative HLS TranscodingUrl becomes absolute and gets an ApiKey', async () => {
    const { api } = mk(() => ({
      data: {
        PlaySessionId: 'ps2',
        MediaSources: [
          {
            Id: 'src1',
            SupportsDirectPlay: false,
            SupportsTranscoding: true,
            TranscodingUrl: '/videos/m1/master.m3u8?MediaSourceId=src1',
            TranscodingSubProtocol: 'hls',
            MediaStreams: [],
          },
        ],
      },
    }))
    const info = await getPlaybackInfo('m1', { api, userId: 'u1' })
    expect(info).toMatchObject({ method: 'Transcode', isHls: true })
    expect(info.url).toBe(
      'http://s:8096/jellyfin/videos/m1/master.m3u8?MediaSourceId=src1&ApiKey=TOK',
    )
  })

  it('does not duplicate an ApiKey already present in the TranscodingUrl', async () => {
    const { api } = mk(() => ({
      data: {
        MediaSources: [
          {
            Id: 's',
            TranscodingUrl: '/videos/m1/master.m3u8?ApiKey=TOK',
            TranscodingSubProtocol: 'hls',
          },
        ],
      },
    }))
    const info = await getPlaybackInfo('m1', { api, userId: 'u1' })
    expect(info.url.match(/api_?key/gi)).toHaveLength(1)
  })

  it('audio uses the universal endpoint', async () => {
    const { api } = mk(() => ({
      data: {
        PlaySessionId: 'ps3',
        MediaSources: [{ Id: 'a1', SupportsDirectPlay: true, MediaStreams: [] }],
      },
    }))
    const info = await getPlaybackInfo('t1', { api, userId: 'u1', kind: 'audio' })
    const u = new URL(info.url)
    expect(u.pathname).toBe('/jellyfin/Audio/t1/universal')
    expect(u.searchParams.get('userId')).toBe('u1')
    expect(u.searchParams.get('deviceId')).toBe('dev1')
    expect(u.searchParams.get('ApiKey')).toBe('TOK')
    expect(u.searchParams.get('transcodingContainer')).toBe('mp3')
    expect(info).toMatchObject({ kind: 'audio', isHls: false, method: 'DirectPlay' })
  })

  it('throws PlaybackError on server refusal or no source', async () => {
    const refused = mk(() => ({ data: { ErrorCode: 'NoCompatibleStream' } }))
    await expect(getPlaybackInfo('m1', { api: refused.api, userId: 'u' })).rejects.toBeInstanceOf(
      PlaybackError,
    )
    const none = mk(() => ({ data: { MediaSources: [] } }))
    await expect(getPlaybackInfo('m1', { api: none.api, userId: 'u' })).rejects.toThrow(
      /No media source/,
    )
    const noStream = mk(() => ({ data: { MediaSources: [{ Id: 's' }] } }))
    await expect(getPlaybackInfo('m1', { api: noStream.api, userId: 'u' })).rejects.toThrow(
      /no playable stream/,
    )
  })
})

describe('URL helpers', () => {
  const { api } = mk(() => ({}))
  it('universalAudioUrl / subtitleUrl', () => {
    expect(universalAudioUrl(api, 'u', 't', { startTimeTicks: 5, maxBitrate: 320000 })).toContain(
      'startTimeTicks=5',
    )
    expect(subtitleUrl(api, 'i', 'm', 7)).toBe(
      'http://s:8096/jellyfin/Videos/i/m/Subtitles/7/0/Stream.vtt?ApiKey=TOK',
    )
  })

  it('streamUrlForCast gives absolute ApiKey URLs for audio, direct video and HLS video', () => {
    const o = { api, userId: 'u' }
    expect(streamUrlForCast('t1', 'audio', o)).toMatch(
      /^http:\/\/s:8096\/jellyfin\/Audio\/t1\/universal\?.*ApiKey=TOK/,
    )
    expect(streamUrlForCast('m1', 'video', o)).toMatch(
      /\/Videos\/m1\/stream\?static=true.*ApiKey=TOK/,
    )
    const hls = streamUrlForCast('m1', 'video', { ...o, transcode: true })
    expect(hls).toMatch(/\/Videos\/m1\/master\.m3u8\?/)
    expect(hls).toContain('videoCodec=h264')
    expect(hls).toContain('audioCodec=aac')
    expect(hls).toContain('ApiKey=TOK')
  })
})

describe('lyrics', () => {
  it('converts ticks to ms and flags synced lyrics', async () => {
    const { api } = mk(() => ({
      data: {
        Lyrics: [
          { Text: 'a', Start: 10_000_000 },
          { Text: 'b', Start: 25_000_000 },
        ],
      },
    }))
    expect(await fetchLyrics(api, 't')).toEqual({
      synced: true,
      lines: [
        { text: 'a', startMs: 1000 },
        { text: 'b', startMs: 2500 },
      ],
    })
  })
  it('unsynced lyrics and no lyrics (404)', async () => {
    const plain = mk(() => ({ data: { Lyrics: [{ Text: 'a' }, { Text: 'b' }] } }))
    expect(await fetchLyrics(plain.api, 't')).toMatchObject({ synced: false })
    const none = mk(() => ({ status: 404 }))
    expect(await fetchLyrics(none.api, 't')).toBeNull()
  })
})

describe('reporting', () => {
  afterEach(() => vi.useRealTimers())
  const target: ReportTarget = {
    itemId: 'm1',
    mediaSourceId: 's1',
    playSessionId: 'ps',
    playMethod: 'DirectPlay',
  }

  it('reportPlaying carries the pause state', async () => {
    const { api, calls } = mk(() => ({ status: 204 }))
    await reportPlaying(target, { positionTicks: 50, isPaused: true }, api)
    expect(calls[0]!.url).toBe('http://s:8096/jellyfin/Sessions/Playing/Progress')
    expect(calls[0]!.data).toMatchObject({
      IsPaused: true,
      EventName: 'pause',
      PositionTicks: 50,
      ItemId: 'm1',
      PlaySessionId: 'ps',
    })
  })

  it('PlaybackReporter: start, 10 s progress, pause, stop', async () => {
    vi.useFakeTimers()
    const { api, calls } = mk(() => ({ status: 204 }))
    let pos = 0
    let paused = false
    const r = new PlaybackReporter(target, () => ({ positionTicks: pos, isPaused: paused }), {
      api,
    })
    await r.start()
    await r.start() // idempotent
    expect(calls.map((c) => c.url.split('/Sessions')[1])).toEqual(['/Playing'])
    pos = 100
    await vi.advanceTimersByTimeAsync(10_000)
    expect(calls.at(-1)!.url).toMatch(/Playing\/Progress$/)
    expect(calls.at(-1)!.data).toMatchObject({ PositionTicks: 100, IsPaused: false })
    await vi.advanceTimersByTimeAsync(9_000)
    expect(calls).toHaveLength(2)
    paused = true
    await r.pause()
    expect(calls.at(-1)!.data).toMatchObject({ IsPaused: true, EventName: 'pause' })
    await r.stop()
    expect(calls.at(-1)!.url).toMatch(/Playing\/Stopped$/)
    const n = calls.length
    await vi.advanceTimersByTimeAsync(30_000)
    expect(calls).toHaveLength(n)
    expect(r.running).toBe(false)
  })

  it('reporting errors go to onError and never throw', async () => {
    const { api } = mk(() => networkError())
    const onError = vi.fn()
    const r = new PlaybackReporter(target, () => ({ positionTicks: 0, isPaused: false }), {
      api,
      onError,
    })
    await expect(r.start()).resolves.toBeUndefined()
    await r.stop()
    expect(onError).toHaveBeenCalledTimes(2)
  })
})
