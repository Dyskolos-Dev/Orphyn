import { describe, expect, it } from 'vitest'
import { BASELINE_SUPPORT, buildDeviceProfile, detectCodecSupport } from './profile'

const dp = (p: ReturnType<typeof buildDeviceProfile>, type: string, container: string) =>
  p.DirectPlayProfiles?.find((d) => d.Type === type && d.Container?.split(',').includes(container))

describe('buildDeviceProfile', () => {
  it('direct plays the common Chromium containers', () => {
    const p = buildDeviceProfile({ support: BASELINE_SUPPORT })
    for (const c of ['mp4', 'webm']) expect(dp(p, 'Video', c)).toBeTruthy()
    for (const c of ['mp3', 'aac', 'flac', 'opus', 'wav', 'ogg', 'm4a']) {
      expect(dp(p, 'Audio', c), c).toBeTruthy()
    }
  })

  it('lists hevc/av1/vp9 only when supported', () => {
    const no = buildDeviceProfile({
      support: { h264: true, hevc: false, vp9: false, av1: false, ac3: false },
    })
    expect(dp(no, 'Video', 'mp4')?.VideoCodec).toBe('h264')
    expect(dp(no, 'Video', 'webm')?.VideoCodec).toBe('vp8')
    const yes = buildDeviceProfile({
      support: { h264: true, hevc: true, vp9: true, av1: true, ac3: false },
    })
    expect(dp(yes, 'Video', 'mp4')?.VideoCodec).toBe('h264,hevc,av1,vp9')
    expect(dp(yes, 'Video', 'webm')?.VideoCodec).toBe('vp8,vp9,av1')
  })

  it('transcodes video to HLS mp4 h264/aac and music to progressive mp3', () => {
    const p = buildDeviceProfile({ support: BASELINE_SUPPORT })
    const video = p.TranscodingProfiles?.find((t) => t.Type === 'Video')
    expect(video).toMatchObject({
      Protocol: 'hls',
      Container: 'mp4',
      VideoCodec: 'h264',
      Context: 'Streaming',
    })
    expect(video?.AudioCodec?.split(',')).toContain('aac')
    expect(p.TranscodingProfiles?.find((t) => t.Type === 'Audio')).toMatchObject({
      Container: 'mp3',
      Protocol: 'http',
    })
  })

  it('serves vtt externally and offers burn-in only when allowed', () => {
    const withBurn = buildDeviceProfile({ support: BASELINE_SUPPORT })
    expect(withBurn.SubtitleProfiles).toContainEqual({ Format: 'vtt', Method: 'External' })
    expect(withBurn.SubtitleProfiles).toContainEqual({ Format: 'pgssub', Method: 'Encode' })
    const without = buildDeviceProfile({ support: BASELINE_SUPPORT, allowBurnIn: false })
    expect(without.SubtitleProfiles?.some((s) => s.Method === 'Encode')).toBe(false)
  })

  it('limits h264 to 8-bit profiles Chromium decodes', () => {
    const p = buildDeviceProfile({ support: BASELINE_SUPPORT })
    const h264 = p.CodecProfiles?.find((c) => c.Codec === 'h264')
    expect(h264?.Conditions?.map((c) => c.Property)).toContain('VideoBitDepth')
  })
})

describe('detectCodecSupport', () => {
  it('maps canPlayType answers', () => {
    const s = detectCodecSupport({
      canPlayType: (t: string) => (t.includes('avc1') || t.includes('vp9') ? 'probably' : ''),
    })
    expect(s).toEqual({ h264: true, vp9: true, hevc: false, av1: false, ac3: false })
  })
})
