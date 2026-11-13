/**
 * Playback preferences used by the video feature. The settings feature will export
 * `usePlaybackPreferences()`; until it lands everything reads preferences through this one
 * function, so swapping the source is a one-line change here.
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
  return DEFAULT_VIDEO_PREFERENCES
}
