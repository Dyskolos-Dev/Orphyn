import { getPlaybackPreferences } from '@renderer/features/settings'

/**
 * The audio quality preference (Settings > Music), read through this single function.
 *
 * @returns maximum streaming bitrate in bits per second, or undefined for "original quality".
 */
export function getAudioMaxBitrate(): number | undefined {
  const quality = getPlaybackPreferences().audioQuality
  return quality === 'original' ? undefined : quality
}
