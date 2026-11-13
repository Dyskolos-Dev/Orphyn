/**
 * The audio quality preference, read through this single function. It will come from
 * `usePlaybackPreferences()` (features/settings) once that lands; until then there is no ceiling
 * (the server direct-plays, or transcodes when the container is unsupported).
 *
 * @returns maximum streaming bitrate in bits per second, or undefined for "original quality".
 */
export function getAudioMaxBitrate(): number | undefined {
  return undefined
}
