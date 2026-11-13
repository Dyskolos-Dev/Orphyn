import type { AudioTrack, PlaybackTracks, SubtitleTrack } from '@renderer/jellyfin'

const ALIASES: Record<string, string> = {
  fr: 'fra',
  fre: 'fra',
  fra: 'fra',
  en: 'eng',
  eng: 'eng',
  de: 'deu',
  ger: 'deu',
  deu: 'deu',
  es: 'spa',
  spa: 'spa',
  it: 'ita',
  ita: 'ita',
  pt: 'por',
  por: 'por',
  ja: 'jpn',
  jpn: 'jpn',
  ko: 'kor',
  kor: 'kor',
  zh: 'zho',
  chi: 'zho',
  zho: 'zho',
  ru: 'rus',
  rus: 'rus',
  nl: 'nld',
  dut: 'nld',
  nld: 'nld',
}

const normLang = (l: string | undefined): string | undefined => {
  if (!l) return undefined
  const k = l.toLowerCase().split(/[-_]/)[0] ?? ''
  return ALIASES[k] ?? k
}

/** True when two ISO 639 codes (2 or 3 letters, bibliographic or terminologic) name the same language. */
export function sameLanguage(a: string | undefined, b: string | undefined): boolean {
  const x = normLang(a)
  const y = normLang(b)
  return !!x && !!y && x === y
}

export interface TrackPrefs {
  audioLanguage?: string
  subtitleLanguage?: string
}

/** Audio stream to start with: preferred language, else the server default, else the default flag, else the first. */
export function pickInitialAudio(
  tracks: PlaybackTracks,
  prefs: TrackPrefs,
): AudioTrack | undefined {
  const { audio } = tracks
  const preferred = prefs.audioLanguage
    ? audio.find((t) => sameLanguage(t.language, prefs.audioLanguage))
    : undefined
  return (
    preferred ??
    audio.find((t) => t.index === tracks.defaultAudioIndex) ??
    audio.find((t) => t.isDefault) ??
    audio[0]
  )
}

/**
 * Subtitle stream to start with. A preferred language wins (non-forced first); otherwise only the
 * server's default selection applies; `undefined` = subtitles off.
 */
export function pickInitialSubtitle(
  tracks: PlaybackTracks,
  prefs: TrackPrefs,
): SubtitleTrack | undefined {
  const { subtitles } = tracks
  if (prefs.subtitleLanguage) {
    const matches = subtitles.filter((t) => sameLanguage(t.language, prefs.subtitleLanguage))
    const pick = matches.find((t) => !t.isForced) ?? matches[0]
    if (pick) return pick
  }
  return subtitles.find((t) => t.index === tracks.defaultSubtitleIndex)
}

/**
 * Does moving from subtitle `from` to `to` (stream indexes, -1/undefined = off) need a new
 * playback request? Only image subtitles (burned in by the server) do, on either side.
 */
export function subtitleSwitchNeedsReload(
  tracks: PlaybackTracks,
  from: number | undefined,
  to: number | undefined,
): boolean {
  if (from === to) return false
  const burn = (i: number | undefined) =>
    i != null && i >= 0 && tracks.subtitles.find((t) => t.index === i)?.burnInRequired === true
  return burn(from) || burn(to)
}

/** Audio can only be switched by asking the server for a new stream. */
export const audioSwitchNeedsReload = (from: number | undefined, to: number): boolean => from !== to
