/**
 * Playback + music preferences, persisted in localStorage under `orphyn.playback.preferences`
 * (one JSON object, see `PlaybackPreferences`). Read them anywhere with `usePlaybackPreferences()`
 * (reactive) or `getPlaybackPreferences()` (imperative, e.g. when building a playback request).
 * Unknown / invalid stored values fall back to the defaults field by field.
 */
import { useSyncExternalStore } from 'react'

export const PLAYBACK_PREFERENCES_KEY = 'orphyn.playback.preferences'

/**
 * - `maxVideoBitrate`: ceiling for video streaming in **bits per second** (pass it as `maxBitrate`
 *   to `getPlaybackInfo`); `null` = no limit (let the server decide).
 * - `preferredAudioLanguage` / `preferredSubtitleLanguage`: ISO 639-2 code as used by Jellyfin
 *   streams (`eng`, `fre`, `jpn`...). Audio `null` = the file's default track. Subtitles `null` =
 *   off. Compare with `languageMatches(pref, track.language)`, which knows the `fre`/`fra` style
 *   aliases.
 * - `autoPlayNext`: start the next episode when one ends.
 * - `audioQuality` (music): `'original'` (direct play / lossless) or a **bits per second** ceiling
 *   for transcoded streams (e.g. 320000).
 */
export interface PlaybackPreferences {
  maxVideoBitrate: number | null
  preferredAudioLanguage: string | null
  preferredSubtitleLanguage: string | null
  autoPlayNext: boolean
  audioQuality: 'original' | number
  /** Music crossfade: on/off (off by default), length in seconds (1-12), and whether to leave
   *  consecutive tracks of the same album gapless. */
  crossfade: boolean
  crossfadeSeconds: number
  crossfadeSkipAlbum: boolean
  /** Music: merge artist variants (feat., &, case, accents...) into one entry. Client-side only. */
  groupArtists: boolean
  /** Music: fall back to LRCLIB for lyrics the server does not have. */
  onlineLyrics: boolean
}

export const CROSSFADE_SECONDS_MIN = 1
export const CROSSFADE_SECONDS_MAX = 12

export const DEFAULT_PLAYBACK_PREFERENCES: PlaybackPreferences = {
  maxVideoBitrate: null,
  preferredAudioLanguage: null,
  preferredSubtitleLanguage: null,
  autoPlayNext: true,
  audioQuality: 'original',
  crossfade: false,
  crossfadeSeconds: 6,
  crossfadeSkipAlbum: true,
  groupArtists: true,
  onlineLyrics: true,
}

/** Video streaming ceilings offered in Settings (bits per second). */
export const VIDEO_BITRATES = [
  120_000_000, 60_000_000, 40_000_000, 20_000_000, 8_000_000, 4_000_000, 2_000_000,
] as const
/** Music transcoding ceilings offered in Settings (bits per second). */
export const AUDIO_BITRATES = [320_000, 256_000, 192_000, 128_000] as const
/** Languages offered in Settings (ISO 639-2). */
export const LANGUAGES = [
  'eng',
  'fre',
  'spa',
  'ger',
  'ita',
  'por',
  'jpn',
  'kor',
  'chi',
  'rus',
] as const

const ALIASES: Record<string, string> = {
  fra: 'fre',
  deu: 'ger',
  zho: 'chi',
  nld: 'dut',
  ces: 'cze',
  ell: 'gre',
  fas: 'per',
  isl: 'ice',
  mkd: 'mac',
  ron: 'rum',
  slk: 'slo',
  sqi: 'alb',
  hye: 'arm',
  kat: 'geo',
  eus: 'baq',
  mya: 'bur',
  msa: 'may',
  bod: 'tib',
  cym: 'wel',
}

/** True when two ISO 639-2 codes name the same language (`fra` ≡ `fre`, case-insensitive). */
export function languageMatches(a: string | null | undefined, b: string | null | undefined) {
  if (!a || !b) return false
  const norm = (c: string) => ALIASES[c.toLowerCase()] ?? c.toLowerCase()
  return norm(a) === norm(b)
}

function sanitize(raw: unknown): PlaybackPreferences {
  const d = DEFAULT_PLAYBACK_PREFERENCES
  const o = raw !== null && typeof raw === 'object' ? (raw as Record<string, unknown>) : {}
  const posNumber = (v: unknown): number | null =>
    typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : null
  const lang = (v: unknown): string | null =>
    typeof v === 'string' && /^[a-z]{3}$/i.test(v) ? v.toLowerCase() : null
  const quality = o['audioQuality']
  return {
    maxVideoBitrate: posNumber(o['maxVideoBitrate']),
    preferredAudioLanguage: lang(o['preferredAudioLanguage']),
    preferredSubtitleLanguage: lang(o['preferredSubtitleLanguage']),
    autoPlayNext: typeof o['autoPlayNext'] === 'boolean' ? o['autoPlayNext'] : d.autoPlayNext,
    audioQuality: quality === 'original' ? 'original' : (posNumber(quality) ?? d.audioQuality),
    crossfade: typeof o['crossfade'] === 'boolean' ? o['crossfade'] : d.crossfade,
    crossfadeSeconds:
      typeof o['crossfadeSeconds'] === 'number' && Number.isFinite(o['crossfadeSeconds'])
        ? Math.min(
            CROSSFADE_SECONDS_MAX,
            Math.max(CROSSFADE_SECONDS_MIN, Math.round(o['crossfadeSeconds'])),
          )
        : d.crossfadeSeconds,
    crossfadeSkipAlbum:
      typeof o['crossfadeSkipAlbum'] === 'boolean' ? o['crossfadeSkipAlbum'] : d.crossfadeSkipAlbum,
    groupArtists: typeof o['groupArtists'] === 'boolean' ? o['groupArtists'] : d.groupArtists,
    onlineLyrics: typeof o['onlineLyrics'] === 'boolean' ? o['onlineLyrics'] : d.onlineLyrics,
  }
}

function read(): PlaybackPreferences {
  try {
    const text = globalThis.localStorage?.getItem(PLAYBACK_PREFERENCES_KEY)
    return sanitize(text ? JSON.parse(text) : null)
  } catch {
    return { ...DEFAULT_PLAYBACK_PREFERENCES }
  }
}

let current = read()
const listeners = new Set<() => void>()

function emit(next: PlaybackPreferences) {
  current = next
  try {
    globalThis.localStorage?.setItem(PLAYBACK_PREFERENCES_KEY, JSON.stringify(next))
  } catch {
    // Storage unavailable: the choice still applies for this session.
  }
  listeners.forEach((l) => l())
}

export function getPlaybackPreferences(): PlaybackPreferences {
  return current
}

export function updatePlaybackPreferences(patch: Partial<PlaybackPreferences>): void {
  emit(sanitize({ ...current, ...patch }))
}

export function resetPlaybackPreferences(): void {
  emit({ ...DEFAULT_PLAYBACK_PREFERENCES })
}

/** Re-reads storage (tests, or another window changed it). */
export function reloadPlaybackPreferences(): void {
  current = read()
  listeners.forEach((l) => l())
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  const onStorage = (e: StorageEvent) => {
    if (e.key === PLAYBACK_PREFERENCES_KEY || e.key === null) reloadPlaybackPreferences()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(listener)
    window.removeEventListener('storage', onStorage)
  }
}

export interface UsePlaybackPreferencesResult extends PlaybackPreferences {
  /** Merge a partial update (persisted immediately, every consumer re-renders). */
  update(patch: Partial<PlaybackPreferences>): void
  reset(): void
}

/** Reactive playback / music preferences. Used by Settings, the video player and the music player. */
export function usePlaybackPreferences(): UsePlaybackPreferencesResult {
  const prefs = useSyncExternalStore(subscribe, getPlaybackPreferences, getPlaybackPreferences)
  return { ...prefs, update: updatePlaybackPreferences, reset: resetPlaybackPreferences }
}
