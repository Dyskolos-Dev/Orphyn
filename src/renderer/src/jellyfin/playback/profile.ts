/**
 * Jellyfin DeviceProfile describing what the app's webview can play natively: WebView2 (Windows),
 * WKWebView (macOS) or WebKitGTK (Linux), each limited by its policy in `@shared/webview`.
 */
import type { DeviceProfile } from '@jellyfin/sdk/lib/generated-client/models'
import {
  currentWebviewEngine,
  mediaPolicyFor,
  restrictSupport,
  type WebviewMediaPolicy,
} from '@shared/webview'

export interface CodecSupport {
  h264: boolean
  hevc: boolean
  vp9: boolean
  av1: boolean
  /** Dolby Digital passthrough is not decodable by Chromium on most builds. */
  ac3: boolean
}

/** Static fallback: what every Chromium build plays. */
export const BASELINE_SUPPORT: CodecSupport = {
  h264: true,
  hevc: false,
  vp9: true,
  av1: true,
  ac3: false,
}

/** Asks the real media stack (hardware-dependent for HEVC/AV1). Falls back to the baseline outside a DOM. */
export function detectCodecSupport(
  video: Pick<HTMLVideoElement, 'canPlayType'> | undefined = typeof document !== 'undefined'
    ? document.createElement('video')
    : undefined,
): CodecSupport {
  if (!video) return { ...BASELINE_SUPPORT }
  const can = (type: string) => video.canPlayType(type) !== ''
  return {
    h264: can('video/mp4; codecs="avc1.640029"'),
    hevc:
      can('video/mp4; codecs="hvc1.1.6.L120.B0"') || can('video/mp4; codecs="hev1.1.6.L120.B0"'),
    vp9: can('video/webm; codecs="vp9"'),
    av1: can('video/mp4; codecs="av01.0.08M.08"') || can('video/webm; codecs="av01.0.08M.08"'),
    ac3: can('audio/mp4; codecs="ac-3"'),
  }
}

export interface ProfileOptions {
  support?: CodecSupport
  /** Offer burn-in (server renders) for image/ASS subtitles. Default true. */
  allowBurnIn?: boolean
  /** Cap on transcoded audio channels. Default 6. */
  maxAudioChannels?: number
  /** Maximum streaming bitrate in bits/s the profile advertises. Default 120 Mbps. */
  maxStreamingBitrate?: number
  /** What the webview may play. Default: the policy of the engine the app runs in. */
  policy?: WebviewMediaPolicy
}

/** DeviceProfile for Orphyn. Direct play whenever the webview can, else HLS fMP4 h264/aac. */
export function buildDeviceProfile(opts: ProfileOptions = {}): DeviceProfile {
  const policy = opts.policy ?? mediaPolicyFor(currentWebviewEngine())
  const s = restrictSupport(opts.support ?? detectCodecSupport(), policy)
  const channels = String(opts.maxAudioChannels ?? 6)
  const requested = opts.maxStreamingBitrate ?? 120_000_000
  const maxBitrate = policy.maxStreamingBitrate
    ? Math.min(requested, policy.maxStreamingBitrate)
    : requested

  const mp4Video = ['h264', s.hevc && 'hevc', s.av1 && 'av1', s.vp9 && 'vp9']
    .filter(Boolean)
    .join(',')
  const webmVideo = ['vp8', s.vp9 && 'vp9', s.av1 && 'av1'].filter(Boolean).join(',')

  const directPlay: NonNullable<DeviceProfile['DirectPlayProfiles']> = [
    {
      Container: 'mp4,m4v',
      Type: 'Video',
      VideoCodec: mp4Video,
      AudioCodec: policy.mp4AudioCodecs.join(','),
    },
    { Container: 'mp3', Type: 'Audio' },
    { Container: 'aac', Type: 'Audio' },
    {
      Container: 'm4a,m4b',
      AudioCodec: policy.mp4AudioCodecs.filter((c) => c !== 'vorbis').join(','),
      Type: 'Audio',
    },
    { Container: 'flac', Type: 'Audio' },
    { Container: 'wav', Type: 'Audio' },
  ]
  if (policy.webm) {
    directPlay.push(
      { Container: 'webm', Type: 'Video', VideoCodec: webmVideo, AudioCodec: 'vorbis,opus' },
      { Container: 'ogg,oga', AudioCodec: 'vorbis,opus,flac', Type: 'Audio' },
      { Container: 'opus', Type: 'Audio' },
      { Container: 'webm,webma', AudioCodec: 'vorbis,opus', Type: 'Audio' },
    )
  }

  const transcoding: NonNullable<DeviceProfile['TranscodingProfiles']> = [
    {
      Container: 'mp4',
      Type: 'Video',
      AudioCodec: 'aac,mp3',
      VideoCodec: 'h264',
      Context: 'Streaming',
      Protocol: 'hls',
      MaxAudioChannels: channels,
      MinSegments: 1,
      BreakOnNonKeyFrames: true,
    },
    {
      // Progressive MP3 for <audio>: hls.js is not needed for music.
      Container: 'mp3',
      Type: 'Audio',
      AudioCodec: 'mp3',
      Context: 'Streaming',
      Protocol: 'http',
      MaxAudioChannels: '2',
    },
  ]

  const subtitles: NonNullable<DeviceProfile['SubtitleProfiles']> = [
    { Format: 'vtt', Method: 'External' },
    { Format: 'srt', Method: 'External' },
    { Format: 'subrip', Method: 'External' },
  ]
  if (opts.allowBurnIn ?? true) {
    for (const f of ['ass', 'ssa', 'pgssub', 'pgs', 'dvdsub', 'dvbsub']) {
      subtitles.push({ Format: f, Method: 'Encode' })
    }
  }

  return {
    Name: policy.profileName,
    MaxStreamingBitrate: maxBitrate,
    MaxStaticBitrate: maxBitrate,
    MusicStreamingTranscodingBitrate: 192_000,
    DirectPlayProfiles: directPlay,
    TranscodingProfiles: transcoding,
    SubtitleProfiles: subtitles,
    CodecProfiles: [
      {
        // Chromium decodes 8-bit h264 up to High profile; 10-bit / Hi422 must be transcoded.
        Type: 'Video',
        Codec: 'h264',
        Conditions: [
          {
            Condition: 'EqualsAny',
            Property: 'VideoProfile',
            Value: 'high|main|baseline|constrained baseline',
            IsRequired: false,
          },
          { Condition: 'LessThanEqual', Property: 'VideoBitDepth', Value: '8', IsRequired: false },
          { Condition: 'NotEquals', Property: 'IsAnamorphic', Value: 'true', IsRequired: false },
        ],
      },
      {
        Type: 'VideoAudio',
        Conditions: [
          {
            Condition: 'LessThanEqual',
            Property: 'AudioChannels',
            Value: channels,
            IsRequired: false,
          },
        ],
      },
    ],
    ResponseProfiles: [{ Type: 'Video', Container: 'm4v', MimeType: 'video/mp4' }],
  } as DeviceProfile
}
