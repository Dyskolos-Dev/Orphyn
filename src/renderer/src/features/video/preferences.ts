import { usePlaybackPreferences } from '@renderer/features/settings'

/**
 * Playback preferences used by the video feature, read from Settings' `usePlaybackPreferences()`.
 */
export interface VideoPlaybackPreferences {
  /** Max streaming bitrate in bits/s; undefined = server / profile default. */
  maxBitrate?: number
  /** Preferred audio language (ISO 639 code, 2 or 3 letters). */
  audioLanguage?: string
  /** Preferred subtitle language; undefined = no subtitles unless forced. */
  subtitleLanguage?: string
  /** Play the next episode automatically. */
  autoPlayNext: boolean
}

export const DEFAULT_VIDEO_PREFERENCES: VideoPlaybackPreferences = {
  autoPlayNext: true,
}

export function useVideoPlaybackPreferences(): VideoPlaybackPreferences {
  const p = usePlaybackPreferences()
  return {
    maxBitrate: p.maxVideoBitrate ?? undefined,
    audioLanguage: p.preferredAudioLanguage ?? undefined,
    subtitleLanguage: p.preferredSubtitleLanguage ?? undefined,
    autoPlayNext: p.autoPlayNext,
  }
}
