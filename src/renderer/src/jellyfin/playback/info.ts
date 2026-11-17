/** Playback resolution: what URL to give a <video>/<audio>, and which tracks it carries. */
import type { Api } from '@jellyfin/sdk'
import type {
  MediaSourceInfo,
  MediaStream,
  PlaybackInfoResponse,
} from '@jellyfin/sdk/lib/generated-client/models'
import { authedUrl, jfRequest } from '../client'
import { requireApi, useSession } from '../session'
import { fetchMediaSegments, type MediaSegment } from '../segments'
import { buildDeviceProfile, type ProfileOptions } from './profile'

export type PlayMethod = 'DirectPlay' | 'DirectStream' | 'Transcode'

export interface AudioTrack {
  /** Jellyfin stream index; pass back as `audioStreamIndex`. */
  index: number
  language?: string
  codec?: string
  channels?: number
  title: string
  isDefault: boolean
}

export interface SubtitleTrack {
  index: number
  language?: string
  codec?: string
  title: string
  isDefault: boolean
  isForced: boolean
  /** WebVTT URL for a `<track>` element; undefined for image subtitles (PGS/DVD), which can only be burned in. */
  vttUrl?: string
  /** True when selecting it requires a new `getPlaybackInfo` with `subtitleStreamIndex` (burn-in). */
  burnInRequired: boolean
}

export interface PlaybackTracks {
  audio: AudioTrack[]
  subtitles: SubtitleTrack[]
  defaultAudioIndex?: number
  defaultSubtitleIndex?: number
}

export interface PlaybackInfo {
  itemId: string
  url: string
  isHls: boolean
  method: PlayMethod
  playSessionId: string
  mediaSource: MediaSourceInfo
  mediaSourceId: string
  tracks: PlaybackTracks
  /** Intro / credits / recap ranges (Jellyfin 10.10+); empty when the server has none. */
  segments?: MediaSegment[]
  /** Start position that was requested (ticks), to seed the player and the reporter. */
  startTimeTicks: number
  kind: 'video' | 'audio'
}

export interface PlaybackOptions {
  kind?: 'video' | 'audio'
  startTimeTicks?: number
  audioStreamIndex?: number
  /** Pass -1 to disable subtitles explicitly. Image subtitles are burned in. */
  subtitleStreamIndex?: number
  /** Bits per second ceiling (user quality setting). */
  maxBitrate?: number
  mediaSourceId?: string
  profile?: ProfileOptions
  /** Override the Api (defaults to the active session). */
  api?: Api
  userId?: string
}

export class PlaybackError extends Error {
  constructor(
    message: string,
    readonly code?: string,
  ) {
    super(message)
    this.name = 'PlaybackError'
  }
}

const TICKS_PER_SECOND = 10_000_000
export const secondsToTicks = (s: number): number => Math.round(s * TICKS_PER_SECOND)
export const ticksToSeconds = (t: number): number => t / TICKS_PER_SECOND

function resolveSession(opts: { api?: Api; userId?: string }): { api: Api; userId: string } {
  const api = opts.api ?? requireApi()
  const userId = opts.userId ?? useSession.getState().account?.userId
  if (!userId) throw new PlaybackError('No active Jellyfin user')
  return { api, userId }
}

/** Container list for GET /Audio/{id}/universal: direct play of anything Chromium decodes. */
export const UNIVERSAL_AUDIO_CONTAINERS =
  'opus,webm|opus,mp3,aac,m4a|aac,m4b|aac,flac,webma,webm|webma,wav,ogg'

/**
 * Universal audio URL for a track: the server direct-plays when the container is supported and
 * otherwise transcodes progressively to MP3. Works in a plain `<audio src>`.
 */
export function universalAudioUrl(
  api: Api,
  userId: string,
  itemId: string,
  opts: { startTimeTicks?: number; maxBitrate?: number } = {},
): string {
  return authedUrl(api, `/Audio/${itemId}/universal`, {
    userId,
    deviceId: api.deviceInfo.id,
    container: UNIVERSAL_AUDIO_CONTAINERS,
    audioCodec: 'mp3',
    transcodingContainer: 'mp3',
    transcodingProtocol: 'http',
    maxStreamingBitrate: opts.maxBitrate,
    startTimeTicks: opts.startTimeTicks || undefined,
    enableRedirection: true,
  })
}

/** WebVTT URL for a text subtitle stream. */
export function subtitleUrl(
  api: Api,
  itemId: string,
  mediaSourceId: string,
  streamIndex: number,
): string {
  return authedUrl(api, `/Videos/${itemId}/${mediaSourceId}/Subtitles/${streamIndex}/0/Stream.vtt`)
}

const IMAGE_SUBTITLE = /^(pgs|pgssub|dvdsub|dvbsub|vobsub|xsub)$/i

function describeStream(s: MediaStream, fallback: string): string {
  return (
    s.DisplayTitle || [s.Language, s.Codec?.toUpperCase()].filter(Boolean).join(' · ') || fallback
  )
}

export function buildTracks(api: Api, itemId: string, source: MediaSourceInfo): PlaybackTracks {
  const streams = source.MediaStreams ?? []
  const audio: AudioTrack[] = streams
    .filter((s) => s.Type === 'Audio')
    .map((s) => ({
      index: s.Index ?? 0,
      language: s.Language ?? undefined,
      codec: s.Codec ?? undefined,
      channels: s.Channels ?? undefined,
      title: describeStream(s, `Audio ${s.Index ?? 0}`),
      isDefault: !!s.IsDefault,
    }))
  const subtitles: SubtitleTrack[] = streams
    .filter((s) => s.Type === 'Subtitle')
    .map((s) => {
      const isImage = IMAGE_SUBTITLE.test(s.Codec ?? '') || s.IsTextSubtitleStream === false
      return {
        index: s.Index ?? 0,
        language: s.Language ?? undefined,
        codec: s.Codec ?? undefined,
        title: describeStream(s, `Subtitle ${s.Index ?? 0}`),
        isDefault: !!s.IsDefault,
        isForced: !!s.IsForced,
        vttUrl: isImage ? undefined : subtitleUrl(api, itemId, source.Id ?? itemId, s.Index ?? 0),
        burnInRequired: isImage,
      }
    })
  return {
    audio,
    subtitles,
    defaultAudioIndex: source.DefaultAudioStreamIndex ?? undefined,
    defaultSubtitleIndex: source.DefaultSubtitleStreamIndex ?? undefined,
  }
}

function resolvePlayUrl(
  api: Api,
  itemId: string,
  source: MediaSourceInfo,
  playSessionId: string,
  kind: 'video' | 'audio',
): { url: string; method: PlayMethod } {
  if (source.SupportsDirectPlay) {
    const path = kind === 'audio' ? `/Audio/${itemId}/stream` : `/Videos/${itemId}/stream`
    return {
      method: 'DirectPlay',
      url: authedUrl(api, path, {
        static: true,
        mediaSourceId: source.Id,
        deviceId: api.deviceInfo.id,
        playSessionId,
        tag: source.ETag ?? undefined,
      }),
    }
  }
  if (source.TranscodingUrl) {
    const hasKey = /[?&](api_key|ApiKey)=/i.test(source.TranscodingUrl)
    const sep = source.TranscodingUrl.includes('?') ? '&' : '?'
    const url = `${api.basePath}${source.TranscodingUrl}${hasKey ? '' : `${sep}ApiKey=${encodeURIComponent(api.accessToken)}`}`
    return { method: source.SupportsDirectStream ? 'DirectStream' : 'Transcode', url }
  }
  throw new PlaybackError('The server offers no playable stream for this item.')
}

/**
 * Asks the server how to play `itemId` with the Chromium device profile and returns a ready URL.
 * Video: direct play, remux, or HLS (fMP4 h264/aac). Audio: the universal endpoint.
 * Re-call it with another `audioStreamIndex` / `subtitleStreamIndex` / `maxBitrate` to switch.
 */
export async function getPlaybackInfo(
  itemId: string,
  opts: PlaybackOptions = {},
): Promise<PlaybackInfo> {
  const { api, userId } = resolveSession(opts)
  const kind = opts.kind ?? 'video'
  const startTimeTicks = opts.startTimeTicks ?? 0
  const profile = buildDeviceProfile(opts.profile)
  const maxStreamingBitrate = opts.maxBitrate ?? profile.MaxStreamingBitrate ?? undefined

  // Segments ride along with the playback info (video only); no segments is a valid answer.
  const infoRequest = jfRequest<PlaybackInfoResponse>(
    api,
    'POST',
    `/Items/${itemId}/PlaybackInfo`,
    {
      params: {
        userId,
        startTimeTicks: startTimeTicks || undefined,
        audioStreamIndex: opts.audioStreamIndex,
        subtitleStreamIndex: opts.subtitleStreamIndex,
        maxStreamingBitrate,
        mediaSourceId: opts.mediaSourceId,
        autoOpenLiveStream: true,
      },
      data: {
        DeviceProfile: profile,
        UserId: userId,
        StartTimeTicks: startTimeTicks || undefined,
        AudioStreamIndex: opts.audioStreamIndex,
        SubtitleStreamIndex: opts.subtitleStreamIndex,
        MaxStreamingBitrate: maxStreamingBitrate,
      },
    },
  )
  const segmentsRequest =
    kind === 'video' ? fetchMediaSegments(api, itemId) : Promise.resolve<MediaSegment[]>([])
  const res = await infoRequest

  if (res.ErrorCode)
    throw new PlaybackError(`Playback refused by the server: ${res.ErrorCode}`, res.ErrorCode)
  const sources = res.MediaSources ?? []
  const source =
    (opts.mediaSourceId && sources.find((s) => s.Id === opts.mediaSourceId)) || sources[0]
  if (!source) throw new PlaybackError('No media source for this item.')
  const playSessionId = res.PlaySessionId ?? ''
  const mediaSourceId = source.Id ?? itemId

  let url: string
  let method: PlayMethod
  if (kind === 'audio') {
    url = universalAudioUrl(api, userId, itemId, { startTimeTicks, maxBitrate: opts.maxBitrate })
    method = source.SupportsDirectPlay ? 'DirectPlay' : 'Transcode'
  } else {
    ;({ url, method } = resolvePlayUrl(api, itemId, source, playSessionId, kind))
  }

  return {
    itemId,
    url,
    isHls: /\.m3u8(\?|$)/i.test(url) || source.TranscodingSubProtocol === 'hls',
    method,
    playSessionId,
    mediaSource: source,
    mediaSourceId,
    tracks: buildTracks(api, itemId, source),
    segments: await segmentsRequest,
    startTimeTicks,
    kind,
  }
}

export interface CastUrlOptions {
  /** Video only: force an HLS h264/aac stream instead of the original file (most TVs need it). */
  transcode?: boolean
  mediaSourceId?: string
  maxBitrate?: number
  api?: Api
  userId?: string
}

/**
 * Absolute URL (with `ApiKey`) a Chromecast / UPnP renderer on the LAN can fetch directly.
 * Note: the renderer must be able to reach the server address stored for the account.
 */
export function streamUrlForCast(
  itemId: string,
  kind: 'video' | 'audio',
  opts: CastUrlOptions = {},
): string {
  const { api, userId } = resolveSession(opts)
  if (kind === 'audio')
    return universalAudioUrl(api, userId, itemId, { maxBitrate: opts.maxBitrate })
  if (opts.transcode) {
    return authedUrl(api, `/Videos/${itemId}/master.m3u8`, {
      mediaSourceId: opts.mediaSourceId ?? itemId,
      deviceId: api.deviceInfo.id,
      videoCodec: 'h264',
      audioCodec: 'aac',
      segmentContainer: 'ts',
      maxAudioChannels: 2,
      minSegments: 1,
      breakOnNonKeyFrames: true,
      videoBitRate: opts.maxBitrate,
    })
  }
  return authedUrl(api, `/Videos/${itemId}/stream`, {
    static: true,
    mediaSourceId: opts.mediaSourceId ?? itemId,
    deviceId: api.deviceInfo.id,
  })
}
