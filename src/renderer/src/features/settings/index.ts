// Settings feature public API.
// Playback / music preferences (localStorage key `orphyn.playback.preferences`): see preferences.ts.
export {
  AUDIO_BITRATES,
  DEFAULT_PLAYBACK_PREFERENCES,
  LANGUAGES,
  PLAYBACK_PREFERENCES_KEY,
  VIDEO_BITRATES,
  getPlaybackPreferences,
  languageMatches,
  resetPlaybackPreferences,
  updatePlaybackPreferences,
  usePlaybackPreferences,
} from './preferences'
export type { PlaybackPreferences, UsePlaybackPreferencesResult } from './preferences'
export { getLanguageChoice, setLanguageChoice } from './language'
export type { LanguageChoice } from './language'
